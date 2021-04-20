use core::convert::TryFrom;
use iso7816::{Instruction, Status};
use apdu_dispatch::{Command, response, applet};
use hid_dispatch::command::Command as FidoCommand;
use ctap_types::{
    authenticator::Error as AuthenticatorError,
    authenticator::Request as AuthenticatorRequest,
    serde::{cbor_serialize},
    ctap1::{Command as U2fCommand},
};

use crate::cbor::{parse_cbor};

use trussed::client;
use fido_authenticator::{Authenticator, UserPresence};
use hid_dispatch::app as hid;

pub struct Fido<UP, T>
where UP: UserPresence,
{
    authenticator: Authenticator<UP, T>,
}

impl<UP, Trussed> Fido<UP, Trussed>
where UP: UserPresence,
      Trussed: client::Client
       + client::P256
       + client::Chacha8Poly1305
       + client::Aes256Cbc
       + client::Sha256
       + client::HmacSha256
       + client::Ed255
       + client::Totp
{
    pub fn new(authenticator: Authenticator<UP, Trussed>) -> Fido<UP, Trussed> {
        Self { authenticator }
    }

    fn response_from_object<T: serde::Serialize>(&mut self, object: Option<T>, reply: &mut response::Data) -> applet::Result {
        reply.resize_to_capacity();
        if let Some(object) = object {
            match cbor_serialize(&object, &mut reply[1..]) {
                Ok(ser) => {
                    let l = ser.len();
                    reply[0] = 0;
                    reply.resize_default(l + 1).unwrap();
                }
                Err(_) => {
                    reply[0] = AuthenticatorError::Other as u8;
                    reply.resize_default(1).unwrap();
                }
            }
        } else {
            reply[0] = 0;
            reply.resize_default(1).unwrap();
        }
        Ok(())
    }

    fn call_authenticator(&mut self, request: &AuthenticatorRequest, reply: &mut response::Data) -> applet::Result {

        let result = self.authenticator.call(request);
        match &result {
            Err(error) => {
                info!("error {}", *error as u8);
                reply.push(*error as u8).ok();
                Ok(())
            }

            Ok(response) => {
                use ctap_types::authenticator::Response;
                match response {
                    Response::Ctap1(_response) => {
                        todo!("CTAP1 responses");
                    }

                    Response::Ctap2(response) => {
                        use ctap_types::authenticator::ctap2::Response;
                        match response {
                            Response::GetInfo(response) => {
                                self.response_from_object(Some(response), reply)
                            },

                            Response::MakeCredential(response) => {
                                self.response_from_object(Some(response), reply)
                            },

                            Response::ClientPin(response) => {
                                self.response_from_object(Some(response), reply)
                            },

                            Response::GetAssertion(response) => {
                                self.response_from_object(Some(response), reply)
                            },

                            Response::GetNextAssertion(response) => {
                                self.response_from_object(Some(response), reply)
                            },

                            Response::CredentialManagement(response) => {
                                self.response_from_object(Some(response), reply)
                            },

                            Response::Reset => {
                                self.response_from_object::<()>(None, reply)
                            },

                            Response::Vendor => {
                                self.response_from_object::<()>(None, reply)
                            },
                        }
                    }
                }
            }
        }
    }

    #[inline(never)]
    fn call_authenticator_u2f_with_bytes(&mut self, request: &mut response::Data) -> applet::Result {
        match &Command::try_from(request) {
            Ok(command) => {
                self.call_authenticator_u2f(command, request)
            },
            _ => {
                Err(Status::IncorrectDataParameter)
            }
        }
    }

    #[inline(never)]
    fn call_authenticator_u2f(&mut self, apdu: &Command, reply: &mut response::Data) -> applet::Result {
        let u2f_command = U2fCommand::try_from(apdu)?;
        let result = self.authenticator.call_u2f(&u2f_command);
        match result {
            Ok(u2f_response) => {
                u2f_response.serialize(reply).unwrap();
                Ok(())
            }
            Err(err)=> Err(err)
        }
    }



}

impl<UP, T> applet::Aid for Fido<UP, T>
where UP: UserPresence,
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
      T: client::Client
       + client::P256
       + client::Chacha8Poly1305
       + client::Aes256Cbc
       + client::Sha256
       + client::HmacSha256
       + client::Ed255
       + client::Totp
{


    fn select(&mut self, _apdu: &Command, reply: &mut response::Data) -> applet::Result {
        // U2F_V2
        reply.extend_from_slice(& [0x55, 0x32, 0x46, 0x5f, 0x56, 0x32,]).unwrap();
        Ok(())
    }

    fn deselect(&mut self) {}

    fn call(&mut self, _type: applet::InterfaceType, apdu: &Command, reply: &mut response::Data) -> applet::Result {
        let instruction = apdu.instruction();

        match instruction {
            Instruction::Unknown(ins) => {
                // TODO need to tidy up these ins codes somewhere
                match ins {
                    // U2F ins codes
                    0x00 | 0x01 | 0x02 => {
                        self.call_authenticator_u2f(apdu, reply)
                    }
                    _ => {
                        match FidoCommand::try_from(ins) {
                            Ok(FidoCommand::Cbor) => {
                                match parse_cbor(apdu.data()) {
                                    Ok(request) => {
                                        info!("parsed cbor");
                                        self.call_authenticator(&request, reply)
                                    }
                                    Err(mapping_error) => {
                                        let authenticator_error: AuthenticatorError = mapping_error.into();
                                        info!("cbor mapping error: {}", authenticator_error as u8);
                                        reply.push(authenticator_error as u8).ok();
                                        Ok(())
                                    }
                                }
                            }
                            Ok(FidoCommand::Msg) => {
                                self.call_authenticator_u2f(apdu, reply)
                            }
                            Ok(FidoCommand::Deselect) => {
                                self.deselect();
                                Ok(())
                            }
                            _ => {
                                info!("Unsupported ins for fido app {:02x}", ins);
                                Err(Status::InstructionNotSupportedOrInvalid)
                            }
                        }
                    }
                }

            }
            _ => {
                info!("Unsupported ins for fido app");
                Err(Status::InstructionNotSupportedOrInvalid)
            }
        }
    }

}

impl<UP, T> hid::App for Fido<UP, T>
where UP: UserPresence,
      T: client::Client
       + client::P256
       + client::Chacha8Poly1305
       + client::Aes256Cbc
       + client::Sha256
       + client::HmacSha256
       + client::Ed255
       + client::Totp
{

    fn commands(&self,) -> &'static [hid::Command] {
        &[ hid::Command::Cbor, hid::Command::Msg ]
    }

    #[inline(never)]
    fn call(&mut self, command: hid::Command, message: &mut hid::Message) -> hid::Response {

        if message.len() < 1 {
            return Err(hid::Error::InvalidLength);
        }
        // info_now!("request: ");
        // blocking::dump_hex(message, message.len());
        match command {
            hid::Command::Cbor => {
                match parse_cbor(message) {
                    Ok(request) => {
                        message.clear();
                        self.call_authenticator(&request, message).ok();
                        Ok(())
                    }
                    Err(mapping_error) => {
                        let authenticator_error: AuthenticatorError = mapping_error.into();
                        info!("authenticator_error: {}", authenticator_error as u8);
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
                let result = self.call_authenticator_u2f_with_bytes(message);
                match result {
                    Ok(()) => {
                        info!("U2F response {} bytes", data.len());
                        // Need to add x9000 success code (normally the apdu-dispatch does this, but
                        // since u2f uses apdus over hid, we must do it here.)
                        message.extend_from_slice(&[0x90, 0x00]).ok();
                    },
                    Err(status) => {
                        let code: [u8; 2] = status.into();
                        info!("U2F error. {}", hex_str!(&code));
                        message.extend_from_slice(&code).ok();
                    },
                }
                Ok(())

            },
        }

    }


}
