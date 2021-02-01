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

use crate::applet::{Applet, Response as AppletResponse, Result as AppletResult};

use iso7816::{
    Aid,
    Command,
    Instruction,
    Response,
    Status,
    response,
    command::FromSliceError,
};

#[derive(Copy, Clone, PartialEq)]
pub enum InterfaceType{
    Contact,
    Contactless,
}

pub enum RequestType {
    Select(Aid),
    GetResponse,
    NewCommand,
    None,
}

use interchange::Responder;
use crate::types::{ContactInterchange, ContactlessInterchange};

#[derive(PartialEq)]
enum RawApduBuffer {
    None,
    Request(Command),
    Response(response::Data),
}

struct ApduBuffer {
    pub raw: RawApduBuffer,
}

impl ApduBuffer {
    fn request(&mut self, command: &Command) {
        match &mut self.raw {
            RawApduBuffer::Request(buffered) => {
                buffered.extend_from_command(command).ok();
            }
            _ => {
                if self.raw != RawApduBuffer::None {
                    info!("Was buffering the last response, but aborting that now for this new request.");
                }
                let mut new_cmd = Command::try_from(&[0,0,0,0]).unwrap();
                new_cmd.extend_from_command(command).ok();
                self.raw = RawApduBuffer::Request(new_cmd);
            }
        }
    }


    fn response(&mut self, response: &response::Data) {
        self.raw = RawApduBuffer::Response(response.clone());
    }

}

pub struct ApduDispatch {
    // or currently_selected_aid, or...
    current_aid: Option<Aid>,
    contact: Responder<ContactInterchange>,
    contactless: Responder<ContactlessInterchange>,
    current_interface: InterfaceType,

    buffer: ApduBuffer,
    was_request_chained: bool,
}

impl ApduDispatch
{
    fn apdu_type(apdu: &Command) -> RequestType {
        if apdu.instruction() == Instruction::Select && (apdu.p1 & 0x04) != 0 {
            RequestType::Select(Aid::try_from_slice(apdu.data()).unwrap())
        } else if apdu.instruction() == Instruction::GetResponse {
            RequestType::GetResponse
        } else {
            RequestType::NewCommand
        }
    }

    pub fn new(
        contact: Responder<ContactInterchange>,
        contactless: Responder<ContactlessInterchange>,
    ) -> ApduDispatch {
        ApduDispatch {
            current_aid: None,
            contact: contact,
            contactless: contactless,
            current_interface: InterfaceType::Contact,
            was_request_chained: false,
            buffer: ApduBuffer {
                raw: RawApduBuffer::None,
            },
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


    #[inline(never)]
    fn buffer_chained_apdu_if_needed(&mut self, command: Command, inferface: InterfaceType) -> RequestType {

        self.current_interface = inferface;
        // iso 7816-4 5.1.1
        // check Apdu level chaining and buffer if necessary.
        if !command.class().chain().not_the_last() {

            let is_chaining = match &self.buffer.raw {
                RawApduBuffer::Request(_) => true,
                _ => false,
            };

            if is_chaining {
                self.buffer.request(&command);

                // Response now needs to be chained.
                self.was_request_chained = true;
                info!("combined chained commands.");

                RequestType::NewCommand
            } else {
                if self.buffer.raw == RawApduBuffer::None {
                    self.was_request_chained = false;
                }
                let apdu_type = Self::apdu_type(&command);
                match Self::apdu_type(&command) {
                    // Keep buffer the same in case of GetResponse
                    RequestType::GetResponse => (),
                    // Overwrite for everything else.
                    _ => self.buffer.request(&command),
                }
                apdu_type
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

            info!("chaining {} bytes", command.data().len());
            self.buffer.request(&command);

            // Nothing for the application to consume yet.
            RequestType::None
        }
    }

    fn parse_apdu(message: &iso7816::command::Data) -> core::result::Result<Command,Response> {

        match Command::try_from(message) {
            Ok(command) => {
                Ok(command)
            },
            Err(_error) => {
                info!("apdu bad");
                match _error {
                    FromSliceError::TooShort => { info!("TooShort"); },
                    FromSliceError::InvalidClass => { info!("InvalidClass"); },
                    FromSliceError::InvalidFirstBodyByteForExtended => { info!("InvalidFirstBodyByteForExtended"); },
                    FromSliceError::CanThisReallyOccur => { info!("CanThisReallyOccur"); },
                }
                Err(Response::Status(Status::UnspecifiedCheckingError))
            }
        }

    }

    #[inline(never)]
    fn check_for_request(&mut self) -> RequestType {
        if !self.busy() {

            // Check to see if we have gotten a message, giving priority to contactless.
            let (message, interface) = if let Some(message) = self.contactless.take_request() {
                (message, InterfaceType::Contactless)
            } else if let Some(message) = self.contact.take_request() {
                (message, InterfaceType::Contact)
            } else {
                return RequestType::None;
            };

            // Parse the message as an APDU.
            match Self::parse_apdu(&message) {
                Ok(command) => {
                    // The Apdu may be standalone or part of a chain.
                    self.buffer_chained_apdu_if_needed(command, interface)
                },
                Err(response) => {
                    // If not a valid APDU, return error and don't pass to app.
                    info!("Invalid apdu");
                    match interface {
                        InterfaceType::Contactless =>
                            self.contactless.respond(response.into_message()).expect("cant respond"),
                        InterfaceType::Contact =>
                            self.contact.respond(response.into_message()).expect("cant respond"),
                    }
                    RequestType::None
                }
            }

        } else {
            RequestType::None
        }
    }

    #[inline(never)]
    fn reply_error (&mut self, status: Status) {
        self.respond(Response::Status(status).into_message());
        self.buffer.raw = RawApduBuffer::None;
    }

    #[inline(never)]
    fn handle_reply(&mut self,) {
        // Consider if we need to reply via chaining method.
        // If the reader is using chaining, we will simply
        // reply 61XX, and put the response in a buffer.
        // It is up to the reader to then send GetResponse
        // requests, to which we will return up to 256 bytes at a time.
        let (new_state, response) = match &mut self.buffer.raw {
            RawApduBuffer::Request(_) | RawApduBuffer::None => {
                info!("Unexpected GetResponse request.");
                (
                    RawApduBuffer::None,
                    Response::Status(Status::UnspecifiedCheckingError).into_message()
                )
            }
            RawApduBuffer::Response(res) => {

                if self.was_request_chained {

                    // Send 256 bytes max at a time.
                    let boundary = core::cmp::min(256, res.len());

                    let to_send = &res[..boundary];
                    let remaining = &res[boundary..];
                    let mut message = response::Data::try_from_slice(to_send).unwrap();
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
                    if return_code == 0x9000 {
                        (
                            RawApduBuffer::None,
                            message
                        )
                    } else {
                        info!("Still {} bytes in response buffer", remaining.len());
                        (
                            RawApduBuffer::Response(response::Data::try_from_slice(remaining).unwrap()),
                            message
                        )
                    }

                } else {
                    // Add success code
                    res.extend_from_slice(&[0x90,00]).ok();
                    (RawApduBuffer::None, res.clone())
                }

            }
        };
        self.buffer.raw = new_state;
        self.respond(response);

    }

    #[inline(never)]
    fn handle_app_response(&mut self, response: &AppletResult) {
        // put message into the response buffer
        match response {
            Ok(AppletResponse::Respond(response)) => {
                info!("buffered the response of {} bytes.", response.len());
                self.buffer.response(response);
                self.handle_reply();
            }
            Ok(AppletResponse::Defer) => {
                // no op
            }
            Err(status) => {
                // Just reply the error immediately.
                info!("buffered applet error");
                self.reply_error(*status);
            }
        }
    }

    #[inline(never)]
    fn handle_app_select<'a>(&mut self, applets: &'a mut [&'a mut dyn Applet], aid: Aid) {
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
            info!("Selected app");
            let result = match &self.buffer.raw {
                RawApduBuffer::Request(apdu) => {
                    applet.select(apdu)
                }
                _ => panic!("Unexpected buffer state."),
            };
            if result.is_ok() {
                self.current_aid = Some(aid);
            }

            self.handle_app_response(&result);


        } else {
            info!("could not find app by aid: {}", hex_str!(&aid));
            self.reply_error(Status::NotFound);
        };

    }


    #[inline(never)]
    fn handle_app_command<'a>(&mut self, applets: &'a mut [&'a mut dyn Applet]) {
        // if there is a selected app, send it the command
        if let Some(applet) = Self::find_applet(self.current_aid.as_ref(), applets) {
            let response = match &self.buffer.raw {
                RawApduBuffer::Request(apdu) => {
                    // TODO this isn't very clear
                    applet.call(self.current_interface, apdu)
                }
                _ => panic!("Unexpected buffer state."),
            };
            self.handle_app_response(& response);

        } else {
            // TODO: correct error?
            self.reply_error(Status::NotFound);
        };
    }

    #[inline(never)]
    fn handle_app_poll<'a>(&mut self, applets: &'a mut [&'a mut dyn Applet]) {
        if let Some(applet) = Self::find_applet(self.current_aid.as_ref(), applets) {
            self.handle_app_response(& applet.poll());
        } else {
            // ideally, we should be able to exclude this case, as there should always be
            // an AID that is selected by default. but this may not be possible, as not all
            // system resources are available during init
        }
    }

    pub fn poll<'a>(
        &mut self,
        applets: &'a mut [&'a mut dyn Applet],
    ) {

        // Only take on one transaction at a time.
        let request_type = self.check_for_request();

        // if there is a new request:
        // - if it's a select, handle appropriately
        // - else pass it on to currently selected app
        // if there is no new request, poll currently selected app
        match request_type {
            // SELECT case
            RequestType::Select(aid) => {
                info!("Select");
                self.handle_app_select(applets,aid);
            }


            RequestType::GetResponse => {
                info!("GetResponse");
                self.handle_reply();
            }

            // command that is not a special command -- goes to applet.
            RequestType::NewCommand => {
                info!("Command");
                self.handle_app_command(applets);
            }

            RequestType::None => {
                self.handle_app_poll(applets);
            }
        }

    }

    #[inline(never)]
    fn respond(&mut self, message: iso7816::response::Data){
        match self.current_interface {
            InterfaceType::Contactless =>
                self.contactless.respond(message).expect("cant respond"),
            InterfaceType::Contact =>
                self.contact.respond(message).expect("cant respond"),
        }
    }
}
