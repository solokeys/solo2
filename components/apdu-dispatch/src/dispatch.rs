//! This "APDU dispatch" consumes APDUs from either a contactless or contact interface, or both.
//! Each APDU will be sent to an "App".  The dispatch will manage selecting and deselecting apps,
//! and will gauruntee only one app will be selected at a time.  Only the selected app will
//! receive APDU's.  Apps are selected based on their AID.
//!
//! Additionally, the APDU dispatch will repeatedly call "poll" on the selected App.  The App
//! can choose to reply at time of APDU, or can defer and reply later (during one of the poll calls).
//!
//! Apps need to implement the Applet trait to be managed.
//!

use crate::applet::{Applet, Response as AppletResponse};

use iso7816::{
    Aid,
    Command,
    Instruction,
    Response,
    Status,
    response,
    command::FromSliceError,
};

#[derive(Copy, Clone)]
pub enum InterfaceType{
    Contact,
    Contactless,
}

pub enum ApduType{
    Select(Aid),
    GetResponse,
    Other,
}

use crate::logger::info;

use interchange::Responder;
use crate::types::{ContactInterchange, ContactlessInterchange};

pub struct ApduDispatch {
    // or currently_selected_aid, or...
    current_aid: Option<Aid>,
    contact: Responder<ContactInterchange>,
    contactless: Responder<ContactlessInterchange>,
    current_interface: InterfaceType,

    chain_buffer: response::Data,
    is_chaining_response: bool,
}

impl ApduDispatch
{
    fn apdu_type(apdu: &Command) -> ApduType {
        if apdu.instruction() == Instruction::Select && (apdu.p1 & 0x04) != 0 {
            ApduType::Select(Aid::from_slice(apdu.data()).unwrap())
        } else if apdu.instruction() == Instruction::GetResponse {
            ApduType::GetResponse
        } else {
            ApduType::Other
        }
    }

    pub fn new(
        contact: Responder<ContactInterchange>,
        contactless: Responder<ContactlessInterchange>,
    ) -> ApduDispatch {
        ApduDispatch{
            current_aid: None,
            contact: contact,
            contactless: contactless,
            current_interface: InterfaceType::Contact,
            chain_buffer: response::Data::new(),
            is_chaining_response: false,
        }
    }

    // It would be nice to store `current_applet` instead of constantly looking up by AID,
    // but that won't work due to ownership rules
    fn find_applet<'a, 'b>(
        aid: Option<&Aid>,
        applets: &'a mut [&'b mut dyn Applet]
    ) -> Option<&'a mut &'b mut dyn Applet> {

        // match aid {
        //     Some(aid) => applets.iter_mut().find(|applet| aid.starts_with(applet.rid())),
        //     None => None,
        // }
        aid.and_then(move |aid|
            applets.iter_mut().find(|applet|
                aid.starts_with(applet.rid())
            )
        )
    }

    fn busy(&self) -> bool {
        // the correctness of this relies on the properties of interchange - requester can only
        // send request in the idle state.
        use interchange::State::*;
        let contactless_busy = match self.contactless.state() {
            Idle | Requested => false,
            _ => true,

        };
        let contact_busy = match self.contact.state() {
            Idle | Requested => false,
            _ => true,

        };
        contactless_busy || contact_busy
    }

    fn buffer_chained_apdu_if_needed(&mut self, command: Command, inferface: InterfaceType) -> Option<Command>{
        self.current_interface = inferface;
        // iso 7816-4 5.1.1
        // check Apdu level chaining and buffer if necessary.
        if command.class().chain().last_or_only() {
            if self.chain_buffer.len() > 0 && !self.is_chaining_response{
                // Merge the chained buffer with the new apdu.
                self.chain_buffer.extend_from_slice(command.data()).unwrap();
                let length: u16 = (self.chain_buffer.len() - 7) as u16;

                self.chain_buffer[0] = command.class().into_inner();
                self.chain_buffer[1] = command.instruction().into();
                self.chain_buffer[2] = command.p1;
                self.chain_buffer[3] = command.p2;
                //   chain_buffer[4] == 0
                self.chain_buffer[5] = ((length & 0xff00) >> 8) as u8;
                self.chain_buffer[6] = (length & 0xff) as u8;

                info!("merging {} bytes", length).ok();
                let merged_apdu = Command::try_from(&self.chain_buffer).unwrap();
                self.chain_buffer.clear();

                // Response now needs to be chained.
                self.is_chaining_response = true;

                Some(merged_apdu)
            } else {
                Some(command)
            }
        } else {
            match inferface {
                // acknowledge
                InterfaceType::Contact => {
                    self.contact.respond(Response::Data(Default::default()).into_message())
                        .expect("Could not respond");
                }
                InterfaceType::Contactless => {
                    self.contactless.respond(Response::Data(Default::default()).into_message())
                        .expect("Could not respond");
                }
            }
            if self.is_chaining_response {
                info!("Was chaining the last response, but aborting that now for this new request.").ok();
                self.is_chaining_response = false;
                self.chain_buffer.clear();
            }
            if self.chain_buffer.len() == 0 {
                // Prepend an extended length apdu header.
                self.chain_buffer.push(0x00).ok();   // cla
                self.chain_buffer.push(0x00).ok();   // ins
                self.chain_buffer.push(0x00).ok();   // p1
                self.chain_buffer.push(0x00).ok();   // p2
                self.chain_buffer.push(0x00).ok();   // 0x00
                self.chain_buffer.push(0x00).ok();   // length upper byte
                self.chain_buffer.push(0x00).ok();   // length lower byte
            }
            info!("chaining {} bytes", command.data().len()).ok();
            self.chain_buffer.extend_from_slice(&command.data()).ok();
            None
        }
    }

    fn parse_apdu(message: &iso7816::command::Data) -> core::result::Result<Command,Response> {

        match Command::try_from(message) {
            Ok(command) => {
                Ok(command)
            },
            Err(_error) => {
                logging::info!("apdu bad").ok();
                match _error {
                    FromSliceError::TooShort => { info!("TooShort").ok(); },
                    FromSliceError::InvalidClass => { info!("InvalidClass").ok(); },
                    FromSliceError::InvalidFirstBodyByteForExtended => { info!("InvalidFirstBodyByteForExtended").ok(); },
                    FromSliceError::CanThisReallyOccur => { info!("CanThisReallyOccur").ok(); },
                }
                Err(Response::Status(Status::UnspecifiedCheckingError))
            }
        }

    }

    fn check_for_request(&mut self) -> Option<Command> {
        if !self.busy() {

            // Check to see if we have gotten a message, giving priority to contactless.
            let (message, interface) = if let Some(message) = self.contactless.take_request() {
                (message, InterfaceType::Contactless)
            } else if let Some(message) = self.contact.take_request() {
                (message, InterfaceType::Contact)
            } else {
                return None;
            };

            // Parse the message as an APDU.
            match Self::parse_apdu(message.as_ref()) {
                Ok(command) => {
                    // The Apdu may be standalone or part of a chain.
                    self.buffer_chained_apdu_if_needed(command, interface)
                },
                Err(response) => {
                    // If not a valid APDU, return error and don't pass to app.
                    match self.current_interface {
                        InterfaceType::Contactless =>
                            self.contactless.respond(response.into_message()).expect("cant respond"),
                        InterfaceType::Contact =>
                            self.contact.respond(response.into_message()).expect("cant respond"),
                    }
                    None
                }
            }

        } else {
            None
        }
    }

    pub fn poll<'a>(
        &mut self,
        applets: &'a mut [&'a mut dyn Applet],
    ) {

        // Only take on one transaction at a time.
        let request = self.check_for_request();

        // if there is a new request:
        // - if it's a select, handle appropriately
        // - else pass it on to currently selected app
        // if there is no new request, poll currently selected app
        let response = match request {
            // have new command APDU
            Some(apdu) => {
                // three cases: SELECT, GET RESPONSE, or Other
                match Self::apdu_type(&apdu) {
                    // SELECT case
                    ApduType::Select(aid) => {
                        // three cases:
                        // - currently selected app has different AID -> deselect it, to give it
                        //   the chance to clear sensitive state
                        // - currently selected app has given AID (typical behaviour will be NOP,
                        //   but pass along anyway) -> do not deselect it first
                        // - no currently selected app
                        //
                        // For PIV, "SELECT" is NOP if it was already selected, but this is
                        // not necessarily the case for other apps

                        // if there is a selected app with a different AID, deselect it
                        if let Some(current_aid) = self.current_aid.as_ref() {
                            if *current_aid != *aid {
                                let applet = Self::find_applet(self.current_aid.as_ref(), applets).unwrap();
                                // for now all applets will be happy with this.
                                applet.deselect();
                                self.current_aid = None;
                            }
                        }

                        // select specified app in any case
                        if let Some(applet) = Self::find_applet(Some(&aid), applets) {
                            let result = applet.select(apdu);
                            if result.is_ok() {
                                self.current_aid = Some(aid);
                            }
                            result
                        } else {
                            Err(Status::NotFound)
                        }

                    }


                    ApduType::GetResponse => {
                        // The reader/host is using chaining.  On behalf of the app,
                        // we will return the response in chunks.
                        if self.chain_buffer.len() == 0 || !self.is_chaining_response {
                            info!("Unexpected GetResponse").ok();
                            Err(Status::UnspecifiedCheckingError)
                        } else {
                            // This is a bit unclear, but am returning this
                            // just to continue the chaining response.
                            Ok(AppletResponse::Respond(Default::default()))
                        }
                    }

                    // command that is not a special command -- goes to applet.
                    ApduType::Other => {
                        // Invalidate the chain_buffer
                        self.chain_buffer.clear();

                        // if there is a selected app, send it the command
                        if let Some(applet) = Self::find_applet(self.current_aid.as_ref(), applets) {
                            applet.call(apdu)
                        } else {
                            // TODO: correct error?
                            Err(Status::NotFound)
                        }
                    }
                }
            }

            // no new command, simply poll the current app
            None => {
                if let Some(applet) = Self::find_applet(self.current_aid.as_ref(), applets) {
                    applet.poll()
                } else {
                    // ideally, we should be able to exclude this case, as there should always be
                    // an AID that is selected by default. but this may not be possible, as not all
                    // system resources are available during init
                    return;
                }
            }
        };

        let message = match response {
            Ok(AppletResponse::Respond(response)) => {

                // Consider if we need to reply via chaining method.
                // If the reader is using chaining, we will simply
                // reply 61XX, and put the response in a buffer.
                // It is up to the reader to then send GetResponse
                // requests, to which we will return up to 256 bytes at a time.
                if self.is_chaining_response {
                    if self.chain_buffer.len() == 0 {
                        self.chain_buffer.extend_from_slice(&response).ok();
                        info!("Putting response of {} bytes into chain buffer", response.len()).ok();
                    }

                    // Send 256 bytes max at a time.
                    let boundary = core::cmp::min(256, self.chain_buffer.len());
                    let to_send = &self.chain_buffer[..boundary];
                    let remaining = &self.chain_buffer[boundary..];
                    let mut message = response::Data::from_slice(to_send).unwrap();
                    let return_code = if remaining.len() > 255 {
                        // XX = 00 indicates more than 255 bytes of data
                        0x6100u16
                    } else if remaining.len() > 0 {
                        0x6100u16 + (remaining.len() as u16)
                    } else {
                        // Last chunk has success code
                        0x9000
                    };
                    message.extend_from_slice(&return_code.to_be_bytes()).ok();
                    self.chain_buffer = response::Data::from_slice(remaining).unwrap();
                    message

                } else {
                    // Just reply normally
                    Response::Data(response).into_message()
                }
            }

            Ok(AppletResponse::Defer) => {
                return;
            }

            Err(status) => {
                info!("applet error").ok();
                Response::Status(status).into_message()
            }
        };

        match self.current_interface {
            InterfaceType::Contactless =>
                self.contactless.respond(message).expect("cant respond"),
            InterfaceType::Contact =>
                self.contact.respond(message).expect("cant respond"),
        }



    }
}
