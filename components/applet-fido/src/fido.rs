use core::convert::TryFrom;
use iso7816::{Command, Instruction, Status};
use heapless::ByteBuf;
use hid_dispatch::command::Command as FidoCommand;
use ctap_types::{
    authenticator::Error as AuthenticatorError,
    authenticator::Request as AuthenticatorRequest,
    serde::{cbor_serialize},
    ctap1::{Command as U2fCommand},
};

use crate::cbor::{parse_cbor};
use crate::logger::{info, blocking};
use logging::hex::*;

use trussed::Client as TrussedClient;
use fido_authenticator::{Authenticator, UserPresence};
use apdu_dispatch::applet;
use hid_dispatch::app as hid;

pub struct Fido<UP, T>
where UP: UserPresence,
      T: TrussedClient
{
    authenticator: Authenticator<UP, T>,
}

impl<UP, TRUSSED> Fido<UP, TRUSSED>
where UP: UserPresence,
      TRUSSED: TrussedClient
{
    pub fn new(authenticator: Authenticator<UP, TRUSSED>) -> Fido<UP, TRUSSED> {
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

    #[inline(never)]
    fn call_authenticator_u2f_with_bytes(&mut self, request: &[u8]) -> applet::Result {
        match &Command::try_from(request) {
            Ok(command) => {
                self.call_authenticator_u2f(command)
            },
            _ => {
                Err(Status::IncorrectDataParameter)
            }
        }
    }

    #[inline(never)]
    fn call_authenticator_u2f(&mut self, apdu: &Command) -> applet::Result {
        let u2f_command = U2fCommand::try_from(apdu)?;
        let result = self.authenticator.call_u2f(&u2f_command);
        match result {
            Ok(u2f_response) => {
                Ok(applet::Response::Respond(u2f_response.serialize()))
            }
            Err(err)=> Err(err)
        }
    }



}

impl<UP, T> applet::Aid for Fido<UP, T>
where UP: UserPresence,
      T: TrussedClient
{
    fn aid(&self) -> &'static [u8] {
        &[ 0xA0, 0x00, 0x00, 0x06, 0x47, 0x2F, 0x00, 0x01 ]
    }
    fn right_truncated_length(&self) -> usize {
        8
    }
}

impl<UP, T> applet::Applet for Fido<UP, T>
where UP: UserPresence,
      T: TrussedClient
{


    fn select(&mut self, _apdu: &Command) -> applet::Result {
        // U2F_V2
        Ok(applet::Response::Respond(ByteBuf::from_slice(
            & [0x55, 0x32, 0x46, 0x5f, 0x56, 0x32,]
        ).unwrap()))
    }

    fn deselect(&mut self) {}

    fn call(&mut self, _type: applet::InterfaceType, apdu: &Command) -> applet::Result {
        let instruction = apdu.instruction();

        match instruction {
            Instruction::Unknown(ins) => {
                // TODO need to tidy up these ins codes somewhere
                match ins {
                    // U2F ins codes
                    0x00 | 0x01 | 0x02 => {
                        self.call_authenticator_u2f(apdu)
                    }
                    _ => {
                        match FidoCommand::try_from(ins) {
                            Ok(FidoCommand::Cbor) => {
                                match parse_cbor(apdu.data()) {
                                    Ok(request) => {
                                        info!("parsed cbor").ok();
                                        self.call_authenticator(&request)
                                    }
                                    Err(mapping_error) => {
                                        let authenticator_error: AuthenticatorError = mapping_error.into();
                                        info!("cbor mapping error: {}", authenticator_error as u8).ok();
                                        Ok(applet::Response::Respond(ByteBuf::from_slice(
                                        & [authenticator_error as u8]
                                        ).unwrap()))
                                    }
                                }
                            }
                            Ok(FidoCommand::Msg) => {
                                self.call_authenticator_u2f(apdu)
                            }
                            Ok(FidoCommand::Deselect) => {
                                self.deselect();
                                Ok(applet::Response::Respond(Default::default()))
                            }
                            _ => {
                                info!("Unsupported ins for fido app {}", ins.hex()).ok();
                                Err(Status::InstructionNotSupportedOrInvalid)
                            }
                        }
                    }
                }

            }
            _ => {
                info!("Unsupported ins for fido app").ok();
                Err(Status::InstructionNotSupportedOrInvalid)
            }
        }
    }

}

impl<UP, T> hid::App for Fido<UP, T>
where UP: UserPresence,
      T: TrussedClient
{

    fn commands(&self,) -> &'static [hid::Command] {
        &[ hid::Command::Cbor, hid::Command::Msg ]
    }

    #[inline(never)]
    fn call(&mut self, command: hid::Command, message: &mut hid::Message) -> hid::Response {

        if message.len() < 1 {
            return Err(hid::Error::InvalidLength);
        }
        // blocking::info!("request: ").ok();
        // blocking::dump_hex(message, message.len()).ok();
        match command {
            hid::Command::Cbor => {
                match parse_cbor(message) {
                    Ok(request) => {
                        let response = self.call_authenticator(&request);
                        match &response {
                            Ok(applet::Response::Respond(buffer)) => {
                                message.clear();
                                message.extend_from_slice(buffer).ok();
                                // blocking::info!("response: ").ok();
                                // blocking::dump_hex(message, message.len()).ok();
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
            },
            // hid::Command::Msg is only other registered command.
            _ => {
                let response = self.call_authenticator_u2f_with_bytes(message);
                message.clear();
                let (response, is_success) = match response {
                    Ok(applet::Response::Respond(data)) => {
                        info!("U2F response {} bytes", data.len()).ok();
                        (data,true)
                    },
                    Err(status) => {
                        let code: u16 = status.into();
                        info!("U2F error. {}", code).ok();
                        (iso7816::Response::Status(status).into_message(), false)
                    },
                    _ => {
                        return Err(hid::Error::NoResponse);
                    }
                };
                // let response = response.into_message();
                message.extend_from_slice(&response).ok();

                if is_success {
                    // Need to add x9000 success code (normally the apdu-dispatch does this, but
                    // since u2f uses apdus over hid, we must do it here.)
                    message.extend_from_slice(&[0x90, 0x00]).ok();
                    // blocking::dump_hex(&message, message.len());
                } else {

                    blocking::dump_hex(&message, message.len()).ok();
                }

                Ok(())

            },
        }

    }


}
