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
};

#[derive(Copy, Clone)]
pub enum InterfaceType{
    Contact,
    Contactless,
}

use crate::logger::info;

use interchange::Responder;

pub type ContactInterchange = usbd_ccid::types::ApduInterchange;
pub type ContactlessInterchange = iso14443::types::ApduInterchange;

pub struct ApduDispatch {
    // or currently_selected_aid, or...
    current_aid: Option<Aid>,
    contact: Responder<ContactInterchange>,
    contactless: Responder<ContactlessInterchange>,
    current_interface: InterfaceType,
}

impl ApduDispatch
{
    fn aid_to_select(apdu: &Command) -> Option<Aid> {
        if apdu.instruction() == Instruction::Select && (apdu.p1 & 0x04) != 0 {
            Some(Aid::from_slice(apdu.data()).unwrap())
        } else {
            None
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

    pub fn poll<'a>(
        &mut self,
        applets: &'a mut [&'a mut dyn Applet],
    ) {

        // Only take on one transaction at a time.
        let request: Option<Command> =
            if !self.busy() {

                // prioritize contactless interface
                if let Some(apdu) = self.contactless.take_request() {
                    self.current_interface = InterfaceType::Contactless;
                    Some(apdu)
                } else if let Some(apdu) = self.contact.take_request() {
                    self.current_interface = InterfaceType::Contact;
                    Some(apdu)
                } else {
                    None
                }

            } else {
                None
            };


        // if there is a new request:
        // - if it's a select, handle appropriately
        // - else pass it on to currently selected app
        // if there is no new request, poll currently selected app
        let response = match request {
            // have new command APDU
            Some(apdu) => {
                // two cases: SELECT or not SELECT
                match Self::aid_to_select(&apdu) {
                    // SELECT case
                    Some(aid) => {
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

                    // command that is not a SELECT command
                    None => {
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

        match response {
            Ok(AppletResponse::Respond(response)) => {
                use InterfaceType::*;
                match self.current_interface {
                    Contactless =>
                        self.contactless.respond(Response::Data(response)).expect("cant respond"),
                    Contact =>
                        self.contact.respond(Response::Data(response)).expect("cant respond"),
                }
            }

            Ok(AppletResponse::Defer) => {}

            Err(status) => {
                info!("applet error").ok();
                use InterfaceType::*;
                match self.current_interface {
                    Contactless =>
                        self.contactless.respond(Response::Status(status)).expect("cant respond"),
                    Contact =>
                        self.contact.respond(Response::Status(status)).expect("cant respond"),
                }
            }
        }
    }
}
