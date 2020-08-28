use core::convert::TryFrom;
use iso7816::{Command, Instruction, Status};
use heapless::ByteBuf;
use hid_dispatch::command::Command as FidoCommand;
use ctap_types::{
    authenticator::Error as AuthenticatorError,
    authenticator::Request as AuthenticatorRequest,
    serde::{cbor_serialize},
};

use crate::cbor::{parse_cbor};
use crate::logger::{info};
use logging::hex::*;

use fido_authenticator::Authenticator;
use apdu_dispatch::applet;
use hid_dispatch::app as hid;

pub struct Fido {
    authenticator: Authenticator,
}

impl Fido {
    pub fn new(authenticator: Authenticator) -> Fido {
        Self { authenticator }
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

    fn call_authenticator(&mut self, request: &AuthenticatorRequest) -> applet::Result {

        let result = self.authenticator.call(request);
        match &result {
            Err(error) => {
                info!("error {}", *error as u8).ok();
                Ok(applet::Response::Respond(ByteBuf::from_slice(
                    & [*error as u8]
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
                                self.response_from_object(Some(response))
                            },

                            Response::MakeCredential(response) => {
                                self.response_from_object(Some(response))
                            },

                            Response::ClientPin(response) => {
                                self.response_from_object(Some(response))
                            },

                            Response::GetAssertion(response) => {
                                self.response_from_object(Some(response))
                            },

                            Response::GetNextAssertion(response) => {
                                self.response_from_object(Some(response))
                            },

                            Response::CredentialManagement(response) => {
                                self.response_from_object(Some(response))
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
                        match parse_cbor(apdu.data()) {
                            Ok(request) => {
                                info!("handled cbor").ok();
                                self.call_authenticator(&request)
                            }
                            Err(mapping_error) => {
                                let authenticator_error: AuthenticatorError = mapping_error.into();
                                info!("cbor mapping error").ok();
                                Ok(applet::Response::Respond(ByteBuf::from_slice(
                                   & [authenticator_error as u8]
                                ).unwrap()))
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

    // fn poll (&mut self) -> applet::Result {
    // }

}

impl hid::App for Fido {

    fn commands(&self,) -> &'static [hid::Command] {
        &[ hid::Command::Cbor,]
    }

    #[inline(never)]
    fn call(&mut self, _command: hid::Command, message: &mut hid::Message) -> hid::Response {

        if message.len() < 1 {
            return Err(hid::Error::InvalidLength);
        }

        match parse_cbor(message) {
            Ok(request) => {
                let response = self.call_authenticator(&request);
                match &response {
                    Ok(applet::Response::Respond(buffer)) => {
                        message.clear();
                        message.extend_from_slice(buffer).ok();
                        Ok(())
                    }
                    _ => {
                        info!("Authenticator ignoring request!").ok();
                        Err(hid::Error::NoResponse)
                    }
                }
            }
            Err(mapping_error) => {
                let authenticator_error: AuthenticatorError = mapping_error.into();
                info!("authenticator_error: {}", authenticator_error as u8).ok();
                message.clear();
                message.extend_from_slice(&[
                    authenticator_error as u8
                ]).ok();
                Ok(())
            }
        }

    }


}