use iso7816::{
    Command as ApduCommand,
    Instruction,
};

use crate::{ByteBuf, consts};

pub const NO_ERROR: u16 = 0x9000;

pub use iso7816::Status as Error;

#[repr(u8)]
#[derive(Copy,Clone,Debug, Eq,PartialEq)]
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
            _ => Err(Error::IncorrectDataParameter),
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone,Debug, Eq,PartialEq)]
pub struct Register {
    pub challenge: ByteBuf<consts::U32>,
    pub app_id: ByteBuf<consts::U32>,
}

#[derive(Clone,Debug, Eq,PartialEq)]
pub struct RegisterResponse {
    pub header_byte: u8,
    pub public_key: ByteBuf<consts::U65>,
    pub key_handle: ByteBuf<consts::U255>,
    pub attestation_certificate: ByteBuf<consts::U1024>,
    pub signature: ByteBuf<consts::U72>,
}

#[derive(Clone,Debug, Eq,PartialEq)]
pub struct Authenticate {
    pub control_byte: ControlByte,
    pub challenge: ByteBuf<consts::U32>,
    pub app_id: ByteBuf<consts::U32>,
    pub key_handle: ByteBuf<consts::U255>,
}

#[derive(Clone,Debug, Eq,PartialEq)]
pub struct AuthenticateResponse {
    user_presence: u8,
    count: u32,
    signature: ByteBuf<consts::U72>,
}

#[derive(Clone,Debug, Eq,PartialEq)]
pub enum Command {
    Register(Register),
    Authenticate(Authenticate),
    Version,
}

#[derive(Clone,Debug, Eq,PartialEq)]
pub enum Response {
    Register(RegisterResponse),
    Authenticate(AuthenticateResponse),
    Version([u8; 6]),
}

impl RegisterResponse {
    pub fn new(
        header_byte: u8, 
        public_key: &crate::cose::EcdhEsHkdf256PublicKey,
        key_handle: &[u8],
        signature: ByteBuf<consts::U72>,
        attestation_certificate: &[u8],
    ) -> Self {

        debug_assert!(key_handle.len()<=255);
        debug_assert!(attestation_certificate.len()<=1024);
        debug_assert!(signature.len()<=72);

        let mut public_key_bytes = ByteBuf::new();
        let mut key_handle_bytes = ByteBuf::new();
        let mut cert_bytes = ByteBuf::new();

        public_key_bytes.push(0x04).unwrap();
        public_key_bytes.extend_from_slice(&public_key.x).unwrap();
        public_key_bytes.extend_from_slice(&public_key.y).unwrap();

        key_handle_bytes.extend_from_slice(key_handle).unwrap();

        cert_bytes.extend_from_slice(attestation_certificate).unwrap();

        Self {
            header_byte: header_byte,
            public_key: public_key_bytes,
            key_handle: key_handle_bytes,
            attestation_certificate: cert_bytes,
            signature: signature,
        }
    }
}

impl AuthenticateResponse {
    pub fn new(
        user_presence: u8, 
        count: u32,
        signature: ByteBuf<consts::U72>,
    ) -> Self {
        Self {
            user_presence: user_presence,
            count: count,
            signature: signature,
        }
    }
}

impl Response {
    pub fn serialize(&self) -> iso7816::response::Data {
        let mut buf = iso7816::response::Data::new();
        match self {
            Response::Register(reg) => {
                buf.push(reg.header_byte).unwrap();
                buf.extend_from_slice(&reg.public_key).unwrap();
                buf.push(reg.key_handle.len() as u8).unwrap();
                buf.extend_from_slice(&reg.key_handle).unwrap();
                buf.extend_from_slice(&reg.attestation_certificate).unwrap();
                buf.extend_from_slice(&reg.signature).unwrap();
                buf
            },
            Response::Authenticate(auth) => {
                buf.push(auth.user_presence).unwrap();
                buf.extend_from_slice(&auth.count.to_be_bytes()).unwrap();
                buf.extend_from_slice(&auth.signature).unwrap();
                buf
            },
            Response::Version(version) => {
                buf.extend_from_slice(version).unwrap();
                buf
            }
        }
    }
}

impl core::convert::TryFrom<&ApduCommand> for Command {
    type Error = Error;
    fn try_from(apdu: &ApduCommand) -> Result<Command> {
        let cla = apdu.class().into_inner();
        let ins = match apdu.instruction() {
            Instruction::Unknown(ins) => ins,
            _ins => 0,
        };
        let p1 = apdu.p1;
        let _p2 = apdu.p2;

        if cla != 0 {
            return Err(Error::ClassNotSupported);
        }

        if ins == 0x3 {
            // for some weird historical reason, [0, 3, 0, 0, 0, 0, 0, 0, 0]
            // is valid to send here.
            return Ok(Command::Version);
        };

        let request = apdu.data();

        match ins {
            // register
            0x1 => {
                if request.len() != 64 {
                    return Err(Error::IncorrectDataParameter);
                }
                Ok(Command::Register(Register {
                    challenge: ByteBuf::from_slice(&request[..32]).unwrap(),
                    app_id: ByteBuf::from_slice(&request[32..]).unwrap(),
                }))
            },

            // authenticate
            0x2 => {
                let control_byte = ControlByte::try_from(p1)?;
                if request.len() < 65 {
                    return Err(Error::IncorrectDataParameter);
                }
                let key_handle_length = request[64] as usize;
                if request.len() != 65 + key_handle_length {
                    return Err(Error::IncorrectDataParameter);
                }
                Ok(Command::Authenticate(Authenticate {
                    control_byte,
                    challenge: ByteBuf::from_slice(&request[..32]).unwrap(),
                    app_id: ByteBuf::from_slice(&request[32..64]).unwrap(),
                    key_handle: ByteBuf::from_slice(&request[65..]).unwrap(),
                }))
            },

            // version
            0x3 => {
                Ok(Command::Version)
            }

            _ => Err(Error::InstructionNotSupportedOrInvalid),
        }
    }
}
