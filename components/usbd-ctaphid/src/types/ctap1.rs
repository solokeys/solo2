//! This is an interoperability layer,
//! allowing authenticators to implement
//! only CTAP2.

use core::convert::TryInto;
// use cortex_m_semihosting::hprintln;
use crate::{
    bytes::{Bytes, consts},
    types::{
        AuthenticatorOptions,
        // MakeCredentialParameters,
        // GetAssertionParameters,
        // PublicKeyCredentialDescriptor,
        // PublicKeyCredentialParameters,
        // PublicKeyCredentialRpEntity,
        // PublicKeyCredentialUserEntity,
    },
};
pub use heapless::{String, Vec};

// pub struct WrongData;

pub const NO_ERROR: u16 = 0x9000;

#[repr(u16)]
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub enum Error {
    ConditionsNotSatisfied = 0x6985,
    WrongData = 0x6A80,
    WrongLength = 0x6700,
    ClaNotSupported = 0x6E00,
    InsNotSupported = 0x6D00,
}

#[repr(u8)]
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub enum ControlByte {
	// Conor:
    // I think U2F check-only maps to FIDO2 MakeCredential with the credID in the excludeList,
    // and pinAuth="" so the request will fail before UP check.
    // I  think this is what the windows hello API does to silently check if a credential is
	// on an authenticator
    CheckOnly = 0x07,
    EnforceUserPresenceAndSign = 0x03,
    DontEnforceUserPresenceAndSign = 0x08,
}

impl core::convert::TryFrom<u8> for ControlByte {
    type Error = Error;

    fn try_from(byte: u8) -> Result<ControlByte> {
        match byte {
            0x07 => Ok(ControlByte::CheckOnly),
            0x03 => Ok(ControlByte::EnforceUserPresenceAndSign),
            0x08 => Ok(ControlByte::DontEnforceUserPresenceAndSign),
            _ => Err(Error::WrongData),
        }
    }
}

// impl Into<[u8; 2]> for Error {
//     fn into(self) -> [u8; 2] {
//         (self as u16).to_be_bytes()
//     }
// }

// #[derive(Clone,Debug,Eq,PartialEq)]
pub type Result<T> = core::result::Result<T, Error>;

// impl From<WrongData> for Error {
//     fn from(_: WrongData) -> Error {
//         Error::WrongData
//     }
// }


#[derive(Clone,Debug,Eq,PartialEq)]
pub struct Register {
    client_data_hash: Bytes<consts::U32>,
    app_id_hash: Bytes<consts::U32>,
    max_response: usize,
}

impl From<ControlByte> for AuthenticatorOptions {
    fn from(control_byte: ControlByte) -> Self {
        AuthenticatorOptions {
            rk: Some(false),
            up: Some(match control_byte {
                ControlByte::CheckOnly => false,
                ControlByte::EnforceUserPresenceAndSign => true,
                ControlByte::DontEnforceUserPresenceAndSign => false,
            }),
            // safety hole?
            uv: Some(false),
        }
    }
}

// impl From<Register> for MakeCredentialParameters {
//     fn from(register: Register) -> Self {
//         let pub_key_cred_params = Vec::new();
//         let key_type = String::new();
//         key_type.push_str("public-key").unwrap();
//         pub_key_cred_params.push(PublicKeyCredentialParameters {
//             alg: -7,
//             key_type,
//         });

//         MakeCredentialParameters {
//             client_data_hash: register.client_data_hash,
//             // uff
//             rp: {
//                 let id = String::new();
//                 id.push_
//                 PublicKeyCredentialRpEntity {
//                     id: String::from(register.app_id_hash),
//                     name: None, url: None,
//                 }
//             },
//             user: PublicKeyCredentialUserEntity {
//                 id: Bytes::new(),
//                 icon: None, name: None, display_name: None,
//             },
//             pub_key_cred_params,
//             exclude_list: None,
//             extensions: None,
//             options: None,
//             pin_auth: None,
//             pin_protocol: None,
//         }
//     }
// }

#[derive(Clone,Debug,Eq,PartialEq)]
pub struct Authenticate {
    control_byte: ControlByte,
    client_data_hash: Bytes<consts::U32>,
    app_id_hash: Bytes<consts::U32>,
    key_handle: Bytes<consts::U255>,
    max_response: usize,
}

#[derive(Clone,Debug,Eq,PartialEq)]
pub enum Command {
    Register(Register),
    Authenticate(Authenticate),
    Version,
}

// U2FHID uses extended length encoding
fn parse_apdu_data(mut remaining: &[u8]) -> Result<(&[u8], usize)> {
    match remaining.len() {
        // Lc = Le = 0
        0 => Ok((&[], 0)),
        // non-zero first byte would indicate short encoding,
        // but U2FHID is always extended length encoding.
        // extended length uses (0,upper byte,lower byte) for
        // lengths; u16_be for the extended lengths, the leading
        // zero to distinguish from short encoding.
        // -> lengths 1 and 2 can't occur
        1 => Err(Error::WrongLength),
        2 => Err(Error::WrongLength),
        _ => {
            if remaining[0] != 0 {
                return Err(Error::WrongData);
            }
            remaining = &remaining[1..];

            let request_length = {
                let first_length = u16::from_be_bytes(remaining[..2].try_into().unwrap()) as usize;
                remaining = &remaining[2..];
                if remaining.len() == 0 {
                    let expected = match first_length {
                        0 => u16::max_value() as usize + 1,
                        non_zero => non_zero,
                    };
                    return Ok((&[], expected));
                }
                first_length
            };

            if remaining.len() < request_length {
                return Err(Error::WrongLength);
            }
            let request = &remaining[..request_length];

            remaining = &remaining[request_length..];
            if remaining.len() == 0 {
                return Ok((request, 0));
            }
            // since Lc is given, Le has no leading zero.
            // single byte would again be short encoding
            if remaining.len() == 1 {
                return Err(Error::WrongData);
            }
            if remaining.len() > 2 {
                return Err(Error::WrongLength);
            }
            let expected = match u16::from_be_bytes(remaining.try_into().unwrap()) as usize {
                0 => u16::max_value() as usize + 1,
                non_zero => non_zero,
            };
            Ok((request, expected))
        },
    }
}

// TODO: From<AssertionResponse> for ...
// public key: 0x4 || uncompressed (x,y) of NIST P-256 public key
// TODO: add "

impl core::convert::TryFrom<&[u8]> for Command {
    type Error = Error;
    fn try_from(apdu: &[u8]) -> Result<Command> {
        if apdu.len() < 4 {
            return Err(Error::WrongData);
        }
        let cla = apdu[0];
        let ins = apdu[1];
        let p1 = apdu[2];
        let _p2 = apdu[3];

        if cla != 0 {
            return Err(Error::ClaNotSupported);
        }

        if ins == 0x3 {
            // for some weird historical reason, [0, 3, 0, 0, 0, 0, 0, 0, 0]
            // is valid to send here.
            return Ok(Command::Version);
        };

        // now we can expect extended length encoded APDUs
        let (request, max_response) = parse_apdu_data(&apdu[4..])?;

        match ins {
            // register
            0x1 => {
                if request.len() != 64 {
                    return Err(Error::WrongData);
                }
                Ok(Command::Register(Register {
                    client_data_hash: Bytes::try_from_slice(&request[..32]).unwrap(),
                    app_id_hash: Bytes::try_from_slice(&request[32..]).unwrap(),
                    max_response,
                }))
            },

            // authenticate
            0x2 => {
                let control_byte = ControlByte::try_from(p1)?;
                if request.len() < 65 {
                    return Err(Error::WrongData);
                }
                let key_handle_length = request[64] as usize;
                if request.len() != 65 + key_handle_length {
                    return Err(Error::WrongData);
                }
                Ok(Command::Authenticate(Authenticate {
                    control_byte,
                    client_data_hash: Bytes::try_from_slice(&request[..32]).unwrap(),
                    app_id_hash: Bytes::try_from_slice(&request[32..]).unwrap(),
                    key_handle: Bytes::try_from_slice(&request[65..]).unwrap(),
                    max_response,
                }))
            },

            // 0x3 => {
            //     Ok(Command::Version)
            // }
            _ => Err(Error::InsNotSupported),
        }
    }
}

// #[derive(Clone,Debug,Eq,PartialEq/*,Serialize,Deserialize*/)]
// pub struct U2fRequest<'a> {
//     pub command: U2fCommand,
//     pub data: &'a [u8],
//     pub expected_length: usize,
// }

