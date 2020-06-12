use core::convert::TryFrom;
use iso7816::{Command, Instruction, response::Result as ResponseResult, Status};
use heapless::ByteBuf;
use usbd_ctaphid::pipe::Command as FidoCommand;

use apdu_manager::{
    Applet,
    Aid,
};

pub struct Fido{
}

impl Fido {
    pub fn new() -> Fido {
        Fido{
        }
    }
}

impl Aid for Fido {
    fn aid(&self) -> &'static [u8] {
        &[ 0xA0, 0x00, 0x00, 0x06, 0x47, 0x2F, 0x00, 0x01 ]
    }
    fn right_truncated_length(&self) -> usize {
        8
    }
}

impl Applet for Fido {


    /// Given parsed APDU for select command.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn select(&mut self, apdu: Command) -> ResponseResult {
        Ok(Default::default())
    }

    /// Deselects the applet.  This may be as a result of another applet getting selected.
    /// It would be a good idea for the applet to use this to reset any sensitive state.
    fn deselect(&mut self) -> Result<(), Status> {
        Ok(())
    }

    /// Given parsed APDU for applet when selected.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn send_recv(&mut self, apdu: Command) -> ResponseResult {
        let instruction = apdu.instruction();

        match instruction {
            Instruction::Unknown(ins) => {
                match FidoCommand::try_from(ins) {
                    Ok(FidoCommand::Cbor) => {
                        Ok(Default::default())
                    }
                    _ => {
                        Err(Status::InstructionNotSupportedOrInvalid)
                    }
                }
            }
            _ => {
                Err(Status::InstructionNotSupportedOrInvalid)
            }
        }

    }
}