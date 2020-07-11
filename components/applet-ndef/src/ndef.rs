use iso7816::{Command, Instruction, Status};
use heapless::ByteBuf;

use apdu_dispatch::applet;

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

impl<'a> applet::Aid for NdefApplet<'a> {
    fn aid(&self) -> &'static [u8] {
        &[0xD2u8, 0x76, 0x00, 0x00,
            0x85, 0x01, 0x01, 0x00,
            ]
    }

    fn right_truncated_length(&self) -> usize {
        8
    }
}

impl<'a> applet::Applet for NdefApplet<'a> {

    fn select(&mut self, _apdu: Command) -> applet::Result {
        Ok(Default::default())
    }

    fn deselect(&mut self) {}

    fn call(&mut self, apdu: Command) -> applet::Result {
        let instruction = apdu.instruction();
        let p1 = apdu.p1;
        let p2 = apdu.p2;
        let expected = apdu.expected();
        let payload = apdu.data();


        match instruction {
            Instruction::Select => {

                if payload.starts_with(&[0xE1u8, 0x03]) {
                    self.reader = &Self::CAPABILITY_CONTAINER;
                    Ok(Default::default())
                } else if payload.starts_with(&[0xE1u8, 0x04]) {
                    self.reader = &Self::NDEF;
                    Ok(Default::default())
                } else {
                    Err(Status::NotFound)
                }
            }
            Instruction::ReadBinary => {
                let offset = (((p1 & 0xef) as usize) << 8) | p2 as usize;
                let len_to_read =
                    if expected as usize > (self.reader.len() - offset) {
                        self.reader.len() - offset
                    } else {
                        if expected > 0 {
                            expected as usize
                        } else {
                            self.reader.len() - offset
                        }
                    };

                Ok(applet::Response::Respond(ByteBuf::from_slice(
                    & self.reader[offset .. offset + len_to_read]
                ).unwrap()))
            }
            _ => {
                Err(Status::ConditionsOfUseNotSatisfied)
            }
        }

    }
}
