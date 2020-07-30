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
use apdu_dispatch::applet;

pub struct Fido {
    interchange: Requester<CtapInterchange>,
}

impl Fido {
    pub fn new(interchange: Requester<CtapInterchange>) -> Fido {
        Self { interchange }
    }

    fn response_from_object<T: serde::Serialize>(&mut self, object: Option<T>) -> applet::Result {
        let mut buffer = ByteBuf::new();
        buffer.resize_to_capacity();

        if let Some(object) = object {
            match cbor_serialize(&object, &mut buffer[1..]) {
                Ok(ser) => {
                    let l = ser.len();
                    buffer[0] = 0;
                    buffer.resize_default(l + 1).unwrap();
                    Ok(applet::Response::Respond(buffer))
                }
                Err(_) => {
                    buffer[0] = AuthenticatorError::Other as u8;
                    buffer.resize_default(1).unwrap();
                    Ok(applet::Response::Respond(buffer))
                }
            }
        } else {
            buffer[0] = 0;
            buffer.resize_default(1).unwrap();
            Ok(applet::Response::Respond(buffer))
        }
    }

}

impl applet::Aid for Fido {
    fn aid(&self) -> &'static [u8] {
        &[ 0xA0, 0x00, 0x00, 0x06, 0x47, 0x2F, 0x00, 0x01 ]
    }
    fn right_truncated_length(&self) -> usize {
        8
    }
}

impl applet::Applet for Fido {


    fn select(&mut self, _apdu: Command) -> applet::Result {
        // U2F_V2
        Ok(applet::Response::Respond(ByteBuf::from_slice(
            & [0x55, 0x32, 0x46, 0x5f, 0x56, 0x32,]
        ).unwrap()))
    }

    fn deselect(&mut self) {}

    fn call(&mut self, apdu: Command) -> applet::Result {
        let instruction = apdu.instruction();

        match instruction {
            Instruction::Unknown(ins) => {
                match FidoCommand::try_from(ins) {
                    Ok(FidoCommand::Cbor) => {
                        match handle_cbor(&mut self.interchange, apdu.data()) {
                            Ok(()) => {
                                info!("handled cbor").ok();
                                Ok(applet::Response::Defer)
                            }
                            Err(CtapMappingError::InvalidCommand(cmd)) => {
                                info!("authenticator command {:?}", cmd).ok();
                                Ok(applet::Response::Respond(ByteBuf::from_slice(
                                   & [AuthenticatorError::InvalidCommand as u8]
                                ).unwrap()))
                            }
                            Err(CtapMappingError::ParsingError(_error)) => {
                                info!("parsing cbor error ").ok();
                                Ok(applet::Response::Respond(ByteBuf::from_slice(
                                   & [AuthenticatorError::InvalidCbor as u8]
                                ).unwrap()))
                            }
                            Err(CtapMappingError::NoData) => {
                                Err(Status::InstructionNotSupportedOrInvalid)
                            }
                        }
                    }
                    _ => {
                        info!("Unsupported ins for fido app {}", ins.hex()).ok();
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

    fn poll (&mut self) -> applet::Result {

        if let Some(result) = self.interchange.take_response() {
            match result {
                Err(error) => {
                    info!("error {}", error as u8).ok();
                    Ok(applet::Response::Respond(ByteBuf::from_slice(
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
                                    self.response_from_object(Some(&response))
                                },

                                Response::MakeCredential(response) => {
                                    self.response_from_object(Some(&response))
                                },

                                Response::ClientPin(response) => {
                                    self.response_from_object(Some(&response))
                                },

                                Response::GetAssertion(response) => {
                                    self.response_from_object(Some(&response))
                                },

                                Response::GetNextAssertion(response) => {
                                    self.response_from_object(Some(&response))
                                },

                                Response::CredentialManagement(response) => {
                                    self.response_from_object(Some(&response))
                                },

                                Response::Reset => {
                                    self.response_from_object::<()>(None)
                                },

                                Response::Vendor => {
                                    self.response_from_object::<()>(None)
                                },
                            }
                        }
                    }
                }
            }


        } else {
            Ok(applet::Response::Defer)
        }

    }


}
