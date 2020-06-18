use core::convert::TryFrom;
use iso7816::{Command, Instruction, Status};
use heapless::ByteBuf;
use usbd_ctaphid::pipe::Command as FidoCommand;
use usbd_ctaphid::pipe::{handle_cbor, CtapMappingError};
use ctap_types::{
    rpc::CtapInterchange,
    authenticator::Error as AuthenticatorError,
    serde::{cbor_serialize},
};

use logging::info;
use logging::hex::*;

use interchange::Requester;
use apdu_manager::{
    Applet,
    Aid,
    AppletResponse,
    ScratchBuffer,
    Result as ResponseResult,
};

pub struct Fido{
    interchange: Requester<CtapInterchange>,
}

impl Fido {
    pub fn new(interchange: Requester<CtapInterchange>) -> Fido {
        Fido{
            interchange,
        }
    }

    fn response_from_object<T: serde::Serialize>(&mut self, buffer: &mut [u8], object: Option<T>) -> ResponseResult {
        if let Some(object) = object {
            match cbor_serialize(&object, &mut buffer[1..]) {
                Ok(ser) => {
                    let l = ser.len();
                    buffer[0] = 0;
                    // buffer[1] = 0;
                    Ok(AppletResponse::Respond(ByteBuf::from_slice(
                        &buffer[.. l + 1]
                    ).unwrap()))
                }
                Err(_) => {
                    Ok(AppletResponse::Respond(ByteBuf::from_slice(
                        & [AuthenticatorError::Other as u8]
                    ).unwrap()))
                }
            }
        } else {
            Ok(AppletResponse::Respond(ByteBuf::from_slice(
                & [0]
            ).unwrap()))
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


    fn select(&mut self, _apdu: Command) -> ResponseResult {
        // U2F_V2
        Ok(AppletResponse::Respond(ByteBuf::from_slice(
            & [0x55, 0x32, 0x46, 0x5f, 0x56, 0x32,]
        ).unwrap()))
    }

    fn deselect(&mut self) -> Result<(), Status> {
        Ok(())
    }

    fn send_recv(&mut self, apdu: Command) -> ResponseResult {
        let instruction = apdu.instruction();

        match instruction {
            Instruction::Unknown(ins) => {
                match FidoCommand::try_from(ins) {
                    Ok(FidoCommand::Cbor) => {
                        match handle_cbor(&mut self.interchange, apdu.data()) {
                            Ok(()) => {
                                info!("handled cbor").ok();
                                Ok(AppletResponse::Defer)
                            }
                            Err(CtapMappingError::InvalidCommand(cmd)) => {
                                info!("authenticator command {:?}", cmd).ok();
                                Ok(AppletResponse::Respond(ByteBuf::from_slice(
                                   & [AuthenticatorError::InvalidCommand as u8]
                                ).unwrap()))
                            }
                            Err(CtapMappingError::ParsingError(_error)) => {
                                info!("parsing cbor error ").ok();
                                Ok(AppletResponse::Respond(ByteBuf::from_slice(
                                   & [AuthenticatorError::InvalidCbor as u8]
                                ).unwrap()))
                            }
                            Err(CtapMappingError::NoData) => {
                                Err(Status::InstructionNotSupportedOrInvalid)
                            }
                        }
                    }
                    _ => {
                        info!("Unsupported ins for fido app {}", logging::hex!(ins)).ok();
                        Err(Status::InstructionNotSupportedOrInvalid)
                    }
                }
            }
            _ => {
                info!("Unsupported ins for fido app").ok();
                Err(Status::InstructionNotSupportedOrInvalid)
            }
        }
    }

    fn poll (&mut self, buffer: &mut ScratchBuffer) -> ResponseResult {

        if let Some(result) = self.interchange.take_response() {
            match result {
                Err(error) => {
                    info!("error {}", error as u8).ok();
                    Ok(AppletResponse::Respond(ByteBuf::from_slice(
                        & [error as u8]
                    ).unwrap()))
                }

                Ok(response) => {
                    use ctap_types::authenticator::Response;
                    match response {
                        Response::Ctap1(_response) => {
                            todo!("CTAP1 responses");
                        }

                        Response::Ctap2(response) => {
                            use ctap_types::authenticator::ctap2::Response;
                            // hprintln!("authnr c2 resp: {:?}", &response).ok();
                            match response {
                                Response::GetInfo(response) => {
                                    self.response_from_object(buffer, Some(&response))
                                },

                                Response::MakeCredential(response) => {
                                    self.response_from_object(buffer, Some(&response))
                                },

                                Response::ClientPin(response) => {
                                    self.response_from_object(buffer, Some(&response))
                                },

                                Response::GetAssertion(response) => {
                                    self.response_from_object(buffer, Some(&response))
                                },

                                Response::GetNextAssertion(response) => {
                                    self.response_from_object(buffer, Some(&response))
                                },

                                Response::CredentialManagement(response) => {
                                    self.response_from_object(buffer, Some(&response))
                                },

                                Response::Reset => {
                                    self.response_from_object::<()>(buffer, None)
                                },

                                Response::Vendor => {
                                    self.response_from_object::<()>(buffer, None)
                                },

                                // _ => {
                                //     todo!("what about all this");
                                // }
                            }
                        }
                    }
                }
            }


        } else {
            Ok(AppletResponse::Defer)
        }

    }


}