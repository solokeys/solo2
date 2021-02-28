use core::convert::{TryFrom, TryInto};

use flexiber::Decodable;
use iso7816::{Command as IsoCommand, command::Data, Instruction, Status};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Command<'l> {
    Select(Select<'l>),
    GetData(GetData),
    Verify(Verify),
    ChangeReference(ChangeReference),
    ChangePin(ChangePin),
    Authenticate(Authenticate),
    PutData(PutData),
    GenerateAsymmetric(GenerateAsymmetric),
}

/// TODO: change into enum
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Select<'l> {
    aid: &'l [u8],
}

impl<'l> TryFrom<&'l Data> for Select<'l> {
    type Error = Status;
    /// We allow ourselves the option of answering to more than just the official PIV AID.
    /// For instance, to offer additional functionality, under our own RID.
    fn try_from(data: &'l Data) -> Result<Self, Self::Error> {
        Ok(match data.as_slice() {
            crate::constants::PIV_AID => Self { aid: data },
            _ => return Err(Status::NotFound),
        })
    }
}


#[derive(Clone, Copy, Eq, PartialEq)]
pub enum GetData {
}

impl TryFrom<&Data> for GetData {
    type Error = Status;
    fn try_from(data: &Data) -> Result<Self, Self::Error> {
        todo!();
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum VerifyKeyReference {
    GlobalPin = 0x00,
    PivPin = 0x80,
    PrimaryFingerOcc = 0x96,
    SecondaryFingerOcc = 0x97,
    PairingCode = 0x98,
}

impl TryFrom<u8> for VerifyKeyReference {
    type Error = Status;
    fn try_from(p2: u8) -> Result<Self, Self::Error> {
        match p2 {
            0x00 => Ok(Self::GlobalPin),
            0x80 => Ok(Self::PivPin),
            0x96 => Err(Status::FunctionNotSupported),
            0x97 => Err(Status::FunctionNotSupported),
            0x98 => Err(Status::FunctionNotSupported),
            _ => Err(Status::KeyReferenceNotFound),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum VerifyParameter1 {
    CheckOrVerify = 0x00,
    Reset = 0xFF,
}

impl TryFrom<u8> for VerifyParameter1 {
    type Error = Status;
    fn try_from(p1: u8) -> Result<Self, Self::Error> {
        match p1 {
            0x00 => Ok(VerifyParameter1::CheckOrVerify),
            0xFF => Ok(VerifyParameter1::Reset),
            _ => Err(Status::IncorrectP1OrP2Parameter),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifyArguments<'l> {
    key_reference: VerifyKeyReference,
    parameter1: VerifyParameter1,
    data: &'l Data
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Verify {
}

impl TryFrom<VerifyArguments<'_>> for Verify {
    type Error = Status;
    fn try_from(arguments: VerifyArguments<'_>) -> Result<Self, Self::Error> {
        todo!();
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum ChangeReferenceKeyReference {
    GlobalPin = 0x00,
    PivPin = 0x80,
    Puk = 0x81,
}

impl TryFrom<u8> for ChangeReferenceKeyReference {
    type Error = Status;
    fn try_from(p2: u8) -> Result<Self, Self::Error> {
        match p2 {
            0x00 => Ok(Self::GlobalPin),
            0x80 => Ok(Self::PivPin),
            0x81 => Ok(Self::Puk),
            _ => Err(Status::KeyReferenceNotFound),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ChangeReferenceArguments<'l> {
    key_reference: ChangeReferenceKeyReference,
    data: &'l Data
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ChangeReference {
}

impl TryFrom<ChangeReferenceArguments<'_>> for ChangeReference {
    type Error = Status;
    fn try_from(arguments: ChangeReferenceArguments<'_>) -> Result<Self, Self::Error> {
        todo!();
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ChangePin {
    padded_pin: [u8; 8],
    puk: [u8;  8],
}

impl TryFrom<&Data> for ChangePin {
    type Error = Status;
    fn try_from(data: &Data) -> Result<Self, Self::Error> {
        if data.len() != 16 {
            return Err(Status::IncorrectDataParameter);
        }
        Ok(Self {
            padded_pin: data[..8].try_into().unwrap(),
            puk: data[8..].try_into().unwrap(),
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum AuthenticateKeyReference {
    SecureMessaging = 0x04,
    Authentication = 0x9a,
    Administration = 0x9b,
    Signature = 0x9c,
    Management = 0x9d,
    CardAuthentication = 0x9e,
    Retired01 = 0x82,
    Retired02 = 0x83,
    Retired03 = 0x84,
    Retired04 = 0x85,
    Retired05 = 0x86,
    Retired06 = 0x87,
    Retired07 = 0x88,
    Retired08 = 0x89,
    Retired09 = 0x8A,
    Retired10 = 0x8B,
    Retired11 = 0x8C,
    Retired12 = 0x8D,
    Retired13 = 0x8E,
    Retired14 = 0x8F,
    Retired15 = 0x90,
    Retired16 = 0x91,
    Retired17 = 0x92,
    Retired18 = 0x93,
    Retired19 = 0x94,
    Retired20 = 0x95,
}

impl TryFrom<u8> for AuthenticateKeyReference {
    type Error = Status;
    fn try_from(p2: u8) -> Result<Self, Self::Error> {
        match p2 {
            0x04 => Ok(Self::SecureMessaging),
            0x9a => Ok(Self::Authentication),
            0x9b => Ok(Self::Administration),
            0x9c => Ok(Self::Signature),
            0x9d => Ok(Self::Management),
            0x9e => Ok(Self::CardAuthentication),
            0x82 => Ok(Self::Retired01),
            0x83 => Ok(Self::Retired02),
            0x84 => Ok(Self::Retired03),
            0x85 => Ok(Self::Retired04),
            0x86 => Ok(Self::Retired05),
            0x87 => Ok(Self::Retired06),
            0x88 => Ok(Self::Retired07),
            0x89 => Ok(Self::Retired08),
            0x8A => Ok(Self::Retired09),
            0x8B => Ok(Self::Retired10),
            0x8C => Ok(Self::Retired11),
            0x8D => Ok(Self::Retired12),
            0x8E => Ok(Self::Retired13),
            0x8F => Ok(Self::Retired14),
            0x90 => Ok(Self::Retired15),
            0x91 => Ok(Self::Retired16),
            0x92 => Ok(Self::Retired17),
            0x93 => Ok(Self::Retired18),
            0x94 => Ok(Self::Retired19),
            0x95 => Ok(Self::Retired20),
            _ => Err(Status::KeyReferenceNotFound),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AuthenticateArguments<'l> {
    unparsed_algorithm: u8,
    key_reference: AuthenticateKeyReference,
    data: &'l Data
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Authenticate {
}

impl TryFrom<AuthenticateArguments<'_>> for Authenticate {
    type Error = Status;
    fn try_from(arguments: AuthenticateArguments<'_>) -> Result<Self, Self::Error> {
        todo!();
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PutData {
}

impl TryFrom<&Data> for PutData {
    type Error = Status;
    fn try_from(data: &Data) -> Result<Self, Self::Error> {
        todo!();
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum GenerateAsymmetricKeyReference {
    SecureMessaging = 0x04,
    Authentication = 0x9a,
    Signature = 0x9c,
    Management = 0x9d,
    CardAuthentication = 0x9e,
}

impl TryFrom<u8> for GenerateAsymmetricKeyReference {
    type Error = Status;
    fn try_from(p2: u8) -> Result<Self, Self::Error> {
        match p2 {
            0x04 => Err(Status::FunctionNotSupported),
            0x9a => Ok(Self::Authentication),
            0x9c => Ok(Self::Signature),
            0x9d => Ok(Self::Management),
            0x9e => Ok(Self::CardAuthentication),
            _ => Err(Status::KeyReferenceNotFound),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct GenerateAsymmetricArguments<'l> {
    key_reference: GenerateAsymmetricKeyReference,
    data: &'l Data
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum GenerateAsymmetric {
}

impl TryFrom<GenerateAsymmetricArguments<'_>> for GenerateAsymmetric {
    type Error = Status;
    fn try_from(arguments: GenerateAsymmetricArguments<'_>) -> Result<Self, Self::Error> {
        todo!();
    }
}

impl<'l> TryFrom<&'l IsoCommand> for Command<'l> {
    type Error = Status;
    /// The first layer of unraveling the iso7816::Command onion.
    ///
    /// The responsibility here is to check (cla, ins, p1, p2) are valid as defined
    /// in the "Command Syntax" boxes of NIST SP 800-73-4, and return early errors.
    ///
    /// The individual piv::Command TryFroms then further interpret these validated parameters.
    fn try_from(command: &'l IsoCommand) -> Result<Self, Self::Error> {
        let (class, instruction, p1, p2) = (command.class(), command.instruction(), command.p1, command.p2);
        let data = command.data();

        if !class.secure_messaging().none() {
            return Err(Status::SecureMessagingNotSupported);
        }

        if class.channel() != Some(0) {
            return Err(Status::LogicalChannelNotSupported);
        }

        // TODO: should we check `command.expected() == 0`, where specified?

        Ok(match (class.into_inner(), instruction, p1, p2) {

            (0x00, Instruction::Select, 0x04, 0x00) => {
                Self::Select(Select::try_from(data)?)
            }

            (0x00, Instruction::GetData, 0x3F, 0xFF) => {
                Self::GetData(GetData::try_from(data)?)
            }

            (0x00, Instruction::Verify, p1, p2) => {
                let parameter1 = VerifyParameter1::try_from(p1)?;
                let key_reference = VerifyKeyReference::try_from(p2)?;
                Self::Verify(Verify::try_from(VerifyArguments { key_reference, parameter1, data })?)
            }

            (0x00, Instruction::ChangeReferenceData, 0x00, p2) => {
                let key_reference = ChangeReferenceKeyReference::try_from(p2)?;
                Self::ChangeReference(ChangeReference::try_from(ChangeReferenceArguments { key_reference, data })?)
            }

            (0x00, Instruction::ResetRetryCounter, 0x00, 0x80) => {
                Self::ChangePin(ChangePin::try_from(data)?)
            }

            (0x00, Instruction::GeneralAuthenticate, p1, p2) => {
                let unparsed_algorithm = p1;
                let key_reference = AuthenticateKeyReference::try_from(p2)?;
                Self::Authenticate(Authenticate::try_from(AuthenticateArguments { unparsed_algorithm, key_reference, data })?)
            }

            (0x00, Instruction::PutData, 0x3F, 0xFF) => {
                Self::PutData(PutData::try_from(data)?)
            }

            (0x00, Instruction::GenerateAsymmetricKeyPair, 0x00, p2) => {
                let key_reference = GenerateAsymmetricKeyReference::try_from(p2)?;
                Self::GenerateAsymmetric(GenerateAsymmetric::try_from(GenerateAsymmetricArguments { key_reference, data })?)
            }

            _ => todo!(),
        })
    }
}
