//! This "APDU manager" consumes APDUs from either a contactless or contact interface, or both.
//! Each APDU will be sent to an "App".  The manager will manage selecting and deselecting apps,
//! and will gauruntee only one app will be selected at a time.  Only the selected app will
//! receive APDU's.  Apps are selected based on their AID.
//!
//! Additionally, the APDU manager will repeatedly call "poll" on the selected App.  The App
//! can choose to reply at time of APDU, or can defer and reply later (during one of the poll calls).
//!
//! Apps need to implement the Applet trait to be managed.
//!

use crate::{
    Applet,
    AppletResponse,
    types,
};

use iso7816::{
    Command,
    Response,
    Instruction,
    Status,
};

#[derive(Copy, Clone)]
pub enum InterfaceType{
    Contact,
    Contactless,
}

use heapless::ByteBuf;
use logging;

use interchange::Responder;

struct AidBuffer {
    pub aid: Option<ByteBuf<heapless::consts::U16>>,
}
// type AidBuffer = Option<ByteBuf<16u8>>;
impl AidBuffer {
    fn new(slice: &[u8]) -> Self {
        AidBuffer{
            aid: Some( ByteBuf::from_slice( slice ).unwrap() )
        }
    }
}
impl Default for AidBuffer {
    fn default() -> Self {
        AidBuffer {
            aid: None
        }
    }
}

pub struct ApduManager {
    selected_aid: AidBuffer,
    contact_interchange: Responder<types::ContactInterchange>,
    contactless_interchange: Responder<types::ContactlessInterchange>,
    last_interface: InterfaceType,
    buffer: [u8; 4096]
}

impl ApduManager
{
    fn is_select(apdu: &Command) -> Option<AidBuffer> {
        if apdu.instruction() == Instruction::Select && (apdu.p1 & 0x04) != 0 {
            Some(AidBuffer::new(apdu.data()))
        } else {
            None
        }
    }

    pub fn new(
        contact_interchange: Responder<types::ContactInterchange>,
        contactless_interchange: Responder<types::ContactlessInterchange>,
    ) -> ApduManager {
        ApduManager{
            selected_aid: Default::default(),
            contact_interchange: contact_interchange,
            contactless_interchange: contactless_interchange,

            last_interface: InterfaceType::Contact,

            // not sure what to make max for nfc messages
            buffer: [0u8; 4096]
        }
    }

    // deselect current applet.
    fn deselect_if_already_selected(&mut self,
        applets: &mut [&mut dyn Applet],
    ) {
        if let Some(aid) = &self.selected_aid.aid {
            for i in 0 .. applets.len() {
                let applet = &mut applets[i];
                if aid.starts_with(applet.rid()) {
                    // For now all applets will be happy with this.
                    applet.deselect().ok();
                    self.selected_aid = Default::default();
                    break;
                }
            }
            if self.selected_aid.aid.is_some() {
                panic!("Tried to deselect nonexistant app");
            }
        }
    }

    // Pick applet from list with matching AID
    fn pick_applet<'a, 'b>(
        aid: &AidBuffer,
        applets: &mut [&'a mut dyn Applet]
    ) -> Option<usize> {

        if let Some(aid) = &aid.aid {
            for i in 0 .. applets.len() {

                let applet_rid = applets[i].rid();

                if aid.starts_with(applet_rid) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn is_no_transaction_ongoing(&self) -> bool {
        let state1 = self.contactless_interchange.state();
        let state2 = self.contact_interchange.state();
        (state1 == interchange::State::Idle || state1 == interchange::State::Requested) &&
        (state2 == interchange::State::Idle || state2 == interchange::State::Requested)
    }

    pub fn poll(
        &mut self,
        // buf: &mut [u8],
        applets: &mut [&mut dyn Applet],
    ) -> () {

        // Only take on one transaction at a time.
        let request =
            if self.is_no_transaction_ongoing() {
                if let Some(apdu) = self.contactless_interchange.take_request() {
                    self.last_interface = InterfaceType::Contactless;
                    Some(apdu)
                } else if let Some(apdu) = self.contact_interchange.take_request() {
                    self.last_interface = InterfaceType::Contact;
                    Some(apdu)
                } else {
                    None
                }

            } else {
                None
            };

        let response = match request {
            Some(apdu) => {
                let maybe_aid = Self::is_select(&apdu);
                let is_select = maybe_aid.is_some();

                let (index,aid) = match maybe_aid {
                    Some(aid) => {
                        (Self::pick_applet(&aid, applets), Some(aid))
                    },
                    _ => {
                        (Self::pick_applet(&self.selected_aid, applets), None)
                    }
                };

                let response = match index {
                    Some(i) => {
                        if is_select {
                            self.deselect_if_already_selected(applets);
                            let res = applets[i].select(apdu);
                            if res.is_ok() {
                                self.selected_aid = aid.unwrap();
                                logging::info!("selected").ok();
                            } else {
                                logging::info!("select rejected by app").ok();
                            }
                            res
                        } else {
                            logging::info!("send recv").ok();
                            applets[i].send_recv(apdu)
                        }
                    }
                    _ => {
                        Err(Status::NotFound)
                    }
                };

                Some(response)
            }
            _ => {
                if let Some(index) = Self::pick_applet(&self.selected_aid, applets) {
                    let applet = &mut applets[index];
                    Some(applet.poll(&mut self.buffer))
                } else {
                    None
                }
            }
        };

        match response {
            Some(Ok(AppletResponse::Respond(response))) => {
                match self.last_interface {
                    InterfaceType::Contactless =>
                        self.contactless_interchange.respond(Response::Data(response)).expect("cant respond"),
                    InterfaceType::Contact=>
                        self.contact_interchange.respond(Response::Data(response)).expect("cant respond"),
                }
            }
            Some(Err(status)) => {
                logging::info!("applet error").ok();
                match self.last_interface {
                    InterfaceType::Contactless =>
                        self.contactless_interchange.respond(Response::Status(status)).expect("cant respond"),
                    InterfaceType::Contact=>
                        self.contact_interchange.respond(Response::Status(status)).expect("cant respond"),
                }
            }
            Some(Ok(AppletResponse::Defer)) => {
            }
            None => {
            }
        }
    }

}