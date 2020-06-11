
use apdu_manager::{
    AidBuffer,
    Apdu,
    Applet,
    Error,
    Ins,
};

pub struct NdefApplet<'a>{
    reader: &'a [u8]
}

impl<'a> NdefApplet<'a> {
    pub const CAPABILITY_CONTAINER: [u8; 15] = [
        0x00, 0x0f, /* CCEN_HI, CCEN_LOW */
        0x20,       /* VERSION */
        0x00, 0x7f, /* MLe_HI, MLe_LOW */
        0x00, 0x7f, /* MLc_HI, MLc_LOW */
        /* TLV */
        0x04,0x06,
        0xe1,0x04,
        0x00,0x7f,
        0x00,0x00,
    ];

    // Externally crafted NDEF URL for "https://solokeys.com/"
    pub const NDEF : [u8; 20] = [
        0x00, 0x12, 0xd1, 0x01, 0x0e, 0x55, 0x04, 0x73, 0x6f, 0x6c, 
        0x6f, 0x6b, 0x65, 0x79, 0x73, 0x2e, 0x63, 0x6f, 0x6d, 0x2f
    ];

    pub fn new() -> NdefApplet<'a> {
        NdefApplet{
            reader: &Self::NDEF,
        }
    }
}

impl<'a> Applet for NdefApplet<'a> {


    /// AID should be 0 padded if needed.
    // const AID: AidBuffer;

    fn aid(&self) -> &AidBuffer {
        &[0xD2u8, 0x76, 0x00, 0x00, 
            0x85, 0x01, 0x01, 0x00, 
            0, 0, 0, 0,
            0,0,0,0]
    }

    /// Given parsed APDU for select command. 
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn select(&mut self, apdu: &mut Apdu) -> Result<u16, Error> {
        Ok(0)
    }

    /// Deselects the applet.  This may be as a result of another applet getting selected.
    /// It would be a good idea for the applet to use this to reset any sensitive state.
    fn deselect(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Given parsed APDU for applet when selected.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn send_recv(&mut self, apdu: &mut Apdu) -> Result<u16, Error>{
        let payload = &apdu.buffer[apdu.offset .. (apdu.offset + apdu.lc as usize)];

        match apdu.ins {
            _ if apdu.ins == Ins::Select as u8 => {
                match payload {
                    &[0xE1u8, 0x03] => {
                        // Capability container
                        self.reader = &Self::CAPABILITY_CONTAINER;
                        Ok(0)
                    }
                    &[0xE1u8, 0x04] => {
                        // NDEF Tag
                        self.reader = &Self::NDEF;
                        Ok(0)
                    }
                    _ => {
                        Err(Error::SwFileNotFound)
                    }
                }
                // payload == &[0x]
            }
            _ if apdu.ins == Ins::ReadBinary as u8 => {
                let offset = (((apdu.p1 & 0xef) as usize) << 8) | apdu.p2 as usize;
                let len_to_read = 
                    if apdu.le as usize > (self.reader.len() - offset) {
                        self.reader.len() - offset 
                    } else {
                        apdu.le as usize
                    };


                for i in 0 .. len_to_read {
                    apdu.buffer[i] = self.reader[offset + i];
                }
                Ok(len_to_read as u16)
            }
            _ => {
                Err(Error::SwCondUseNotSatisfied)
            }
        }

    }
}