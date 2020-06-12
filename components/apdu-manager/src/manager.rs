use nb;
use crate::{
    Applet,
    ApduSource,
    SourceError,
};

use iso7816::{
    Command,
    response::Result as ResponseResult,
    Instruction,
    Status,
    command::FromSliceError,
};
use heapless::ByteBuf;
use logging;
use logging::info;

// type AidBuffer = [u8; 16];

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

pub enum ApduStatus {
    NotSelect,
}

pub struct ApduManager {
    selected_aid: AidBuffer,
}

impl ApduManager
{
    fn is_select(apdu: &Command) -> Result<AidBuffer, ApduStatus> {
        let mut aid = [0u8; 16];
        if apdu.instruction() == Instruction::Select && (apdu.p1 & 0x04) != 0{
            Ok(AidBuffer::new(apdu.data()))
        } else {
            Err(ApduStatus::NotSelect)
        }
    }

    pub fn new() -> ApduManager {
        ApduManager{
            selected_aid: Default::default(),
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

    pub fn poll(
        &mut self,
        buf: &mut [u8],
        source: &mut impl ApduSource,
        applets: &mut [&mut dyn Applet],
    ) -> nb::Result<(), SourceError> {


        let len = source.read_apdu(buf)?;

        let apdu = match Command::try_from(&buf[0 .. len as usize]) {
            Ok(command) => command,
            Err(_error) => {
                // logging::info!("could not parse command from APDU, ignoring.").ok();
                // logging::info!("{:?}", &_error).ok();
                logging::info!("apdu bad").ok();
                match _error {
                    FromSliceError::TooShort => { info!("TooShort").ok(); },
                    FromSliceError::InvalidClass => { info!("InvalidClass").ok(); },
                    FromSliceError::InvalidFirstBodyByteForExtended => { info!("InvalidFirstBodyByteForExtended").ok(); },
                    FromSliceError::CanThisReallyOccur => { info!("CanThisReallyOccur").ok(); },
                }

                source.send_apdu(Status::UnspecifiedCheckingError, &[])?;
                return Ok(());
            }
        };

        // logging::info!("apdu ok").ok();
        let maybe_aid = Self::is_select(&apdu);
        let is_select = maybe_aid.is_ok();

        let index = match maybe_aid {
            Ok(aid) => {
                Self::pick_applet(&aid, applets)
            },
            _ => {
                Self::pick_applet(&self.selected_aid, applets)
            }
        };

        match index {
            Some(i) => {
                // logging::info!("applet? {}", i).ok();
                let applet = &mut applets[i];
                let aid = AidBuffer::new(applet.aid());

                let applet_response = if is_select {
                    applet.select(apdu)
                } else {
                    applet.send_recv(apdu)
                };

                if applet_response.is_ok() {

                    // logging::info!("applet ok").ok();
                    if is_select {
                        self.deselect_if_already_selected(applets);
                        self.selected_aid = aid;
                    }
                    source.send_apdu(Status::Success, &applet_response.ok().unwrap())?;
                }
                else {
                    logging::info!("applet error").ok();
                    source.send_apdu(applet_response.err().unwrap(), &[])?;
                }
            }
            None => {
                logging::info!("No applet").ok();
                source.send_apdu(Status::NotFound, &[])?;
            }
        }



        // logging::dump_hex(&buf, len as usize);

        Ok(())
    }
}