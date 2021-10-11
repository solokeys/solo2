use iso7816::{Instruction, Status};
use apdu_dispatch::{Command, response, app, command::SIZE as CommandSize, response::SIZE as ResponseSize};

pub struct App<'a>{
    reader: &'a [u8]
}

impl<'a> App<'a> {
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

    pub const NDEF : [u8; 20] = [
        0x00, 0x12, /* two-byte length */
        0xd1, /* TNF: well-known + flags */
        0x01, /* payload type length */
        0x0e, /* payload data length */
        0x55, /* payload type: U = URL */
        0x02, /* https://www. */
        0x6e, 0x69, 0x74, 0x72, 0x6f, 0x6b, 0x65, 0x79, 0x2e, 0x63, 0x6f, 0x6d, 0x2f, /* nitrokey.com/ */
    ];

    pub fn new() -> App<'a> {
        App{
            reader: &Self::NDEF,
        }
    }
}

impl<'a> iso7816::App for App<'a> {
    fn aid(&self) -> iso7816::Aid {
        iso7816::Aid::new(&[0xD2u8, 0x76, 0x00, 0x00, 0x85, 0x01, 0x01])
    }
}

impl<'a> app::App<CommandSize, ResponseSize> for App<'a> {

    fn select(&mut self, _apdu: &Command, _reply: &mut response::Data) -> app::Result {
        Ok(())
    }

    fn deselect(&mut self) {}

    fn call(&mut self, _type: app::Interface, apdu: &Command, reply: &mut response::Data) -> app::Result {
        let instruction = apdu.instruction();
        let p1 = apdu.p1;
        let p2 = apdu.p2;
        let expected = apdu.expected();
        let payload = apdu.data();


        match instruction {
            Instruction::Select => {

                if payload.starts_with(&[0xE1u8, 0x03]) {
                    self.reader = &Self::CAPABILITY_CONTAINER;
                    Ok(())
                } else if payload.starts_with(&[0xE1u8, 0x04]) {
                    self.reader = &Self::NDEF;
                    Ok(())
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

                reply.extend_from_slice(& self.reader[offset .. offset + len_to_read]).ok();
                Ok(())
            }
            _ => {
                Err(Status::ConditionsOfUseNotSatisfied)
            }
        }

    }
}
