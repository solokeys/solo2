use nb;
use crate::{
    AidBuffer,
    Applet,
    ApduSource,
    SourceError,
    Error,
    Ins,
};
use crate::Apdu;

use logging;

pub struct ApduManager {
    selected_aid: AidBuffer,
}

impl ApduManager
{
    fn is_select(apdu: &Apdu) -> (bool, AidBuffer) {
        let mut aid = [0u8; 16];
        if apdu.ins == (Ins::Select as u8) && (apdu.p1 & 0x04) != 0{
            for i in 0 .. core::cmp::min(apdu.lc as usize, aid.len()) {
                aid[i] = apdu.buffer[apdu.offset + i as usize];
            }
            (true, aid)
        } else {
            (false, aid)
        }
    }

    pub fn new() -> ApduManager {
        ApduManager{
            selected_aid: [0u8; 16],
        }
    }

    // deselect current applet.
    fn deselect(&mut self,
        applets: &mut [&mut dyn Applet],
    ) {
        for i in 0 .. applets.len() {
            let applet = &mut applets[i];
            if &self.selected_aid == applet.aid() {
                // For now all applets will be happy with this.
                applet.deselect().ok();
                break;
            }
        }
        for i in 0 .. self.selected_aid.len() {
            self.selected_aid[i] = 0;
        }
    }

    // Pick applet from list with matching AID
    fn pick_applet<'a, 'b>(
        aid: &AidBuffer,
        applets: &mut [&'a mut dyn Applet]
    ) -> Option<usize> {
        for i in 0 .. applets.len() {

            let applet_aid = applets[i].aid();

            if  applet_aid == aid {
                return Some(i);
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

        let apdu = Apdu::new(buf, len as usize);
        
        if apdu.is_ok() {
            // logging::info!("apdu ok").ok();
            let mut apdu = apdu.ok().unwrap();
            let (is_select, aid_to_select) = Self::is_select(&apdu);

            let index = if is_select {
                Self::pick_applet(&aid_to_select, applets)
            } else {
                Self::pick_applet(&self.selected_aid, applets)
            };

            match index {
                Some(i) => {
                    // logging::info!("applet? {}", i).ok();
                    let applet = &mut applets[i];

                    let applet_response = if is_select {
                        applet.select(&mut apdu)
                    } else {
                        applet.send_recv(&mut apdu)
                    };

                    if applet_response.is_ok() {

                        // logging::info!("applet ok").ok();
                        if is_select {
                            self.deselect(applets);
                            self.selected_aid.copy_from_slice(&aid_to_select);
                        }
                        source.send_apdu(Error::Success, &apdu.buffer[0 .. applet_response.unwrap() as usize])?;
                    }
                    else {
                        logging::info!("applet error").ok();
                        source.send_apdu(applet_response.err().unwrap(), &[])?;
                    }
                }
                None => {
                    logging::info!("No applet").ok();
                    source.send_apdu(Error::SwFileNotFound, &[])?;
                }
            }


        } else {
            logging::info!("apdu bad").ok();
            source.send_apdu(Error::SwUnknown, &[])?;
        }

        // logging::dump_hex(&buf, len as usize);

        Ok(())
    }
}