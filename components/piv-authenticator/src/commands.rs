//! Parsed PIV commands.
//!
//! The types here should enforce all restrictions in the spec (such as padded_piv_pin.len() == 8),
//! but no implementation-specific ones (such as "GlobalPin not supported").

use core::convert::{TryFrom, TryInto};

// use flexiber::Decodable;
use iso7816::{Instruction, Status};
use apdu_dispatch::{Command as IsoCommand, command::Data};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Command<'l> {
    /// Select the application
    ///
    /// Resets security indicators if we are implicitly deselected.
    Select(Select<'l>),
    /// Get a data object / container.
    GetData(GetData),
    /// Check PIN
    ///
    /// This verifies that the sent PIN (global or PIV) is correct.
    ///
    /// In principle, other key references (biometric, pairing code) could
    /// be verified, but this is not implemented.
    Verify(Verify),
    /// Change PIN or PUK
    ChangeReference(ChangeReference),
    /// If the PIN is blocked, reset it using the PUK
    ResetPinRetries(ResetPinRetries),
    /// The most general purpose method, performing actual cryptographic operations
    ///
    /// In particular, this can also decrypt or similar.
    Authenticate(Authenticate),
    /// Store a data object / container.
    PutData(PutData),
    GenerateAsymmetric(GenerateAsymmetric),
}

impl<'l> Command<'l> {
    /// Core method, constructs a PIV command, if the iso7816::Command is valid.
    ///
    /// Inherent method re-exposing the `TryFrom` implementation.
    pub fn try_from(command: &'l IsoCommand) -> Result<Self, Status> {
        command.try_into()
    }
}

/// TODO: change into enum
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Select<'l> {
    pub aid: &'l [u8],
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
        // If the PIV Card Application does not contain the Discovery Object as described in Part 1,
        // then no other key reference shall be able to be verified by the PIV Card Application VERIFY command.
        match p2 {
            0x00 => Ok(Self::GlobalPin),
            // 0x00 => Err(Status::FunctionNotSupported),
            0x80 => Ok(Self::PivPin),
            0x96 => Ok(Self::PrimaryFingerOcc),
            0x97 => Ok(Self::SecondaryFingerOcc),
            0x98 => Ok(Self::PairingCode),
            // 0x96 => Err(Status::FunctionNotSupported),
            // 0x97 => Err(Status::FunctionNotSupported),
            // 0x98 => Err(Status::FunctionNotSupported),
            _ => Err(Status::KeyReferenceNotFound),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifyLogout(bool);

impl TryFrom<u8> for VerifyLogout {
    type Error = Status;
    fn try_from(p1: u8) -> Result<Self, Self::Error> {
        match p1 {
            0x00 => Ok(Self(false)),
            0xFF => Ok(Self(true)),
            _ => Err(Status::IncorrectP1OrP2Parameter),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifyArguments<'l> {
    pub key_reference: VerifyKeyReference,
    pub logout: VerifyLogout,
    pub data: &'l Data
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum VerifyLogin {
    PivPin([u8; 8]),
    GlobalPin([u8; 8]),
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Verify {
    Login(VerifyLogin),
    Logout(VerifyKeyReference),
    Status(VerifyKeyReference),
}

impl TryFrom<VerifyArguments<'_>> for Verify {
    type Error = Status;
    fn try_from(arguments: VerifyArguments<'_>) -> Result<Self, Self::Error> {
        let VerifyArguments { key_reference, logout, data } = arguments;
        if key_reference != VerifyKeyReference::PivPin {
            return Err(Status::FunctionNotSupported);
        }
        Ok(match (logout.0, data.len()) {
            (false, 0) => Verify::Status(key_reference),
            (false, 8) => Verify::Login(VerifyLogin::PivPin(data.as_slice().try_into().map_err(|_| Status::IncorrectDataParameter)?)),
            (false, _) => return Err(Status::IncorrectDataParameter),
            (true, 0) => Verify::Logout(key_reference),
            (true, _) => return Err(Status::IncorrectDataParameter),
        })
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
    pub key_reference: ChangeReferenceKeyReference,
    pub data: &'l Data
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
pub struct ResetPinRetries {
    pub padded_pin: [u8; 8],
    pub puk: [u8;  8],
}

impl TryFrom<&Data> for ResetPinRetries {
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
    /// To allow the authenticator to have additional algorithms beyond NIST SP 800-78-4,
    /// this is passed through as-is.
    pub unparsed_algorithm: u8,
    pub key_reference: AuthenticateKeyReference,
    pub data: &'l Data
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
    pub key_reference: GenerateAsymmetricKeyReference,
    pub data: &'l Data
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
                let logout = VerifyLogout::try_from(p1)?;
                let key_reference = VerifyKeyReference::try_from(p2)?;
                Self::Verify(Verify::try_from(VerifyArguments { key_reference, logout, data })?)
            }

            (0x00, Instruction::ChangeReferenceData, 0x00, p2) => {
                let key_reference = ChangeReferenceKeyReference::try_from(p2)?;
                Self::ChangeReference(ChangeReference::try_from(ChangeReferenceArguments { key_reference, data })?)
            }

            (0x00, Instruction::ResetRetryCounter, 0x00, 0x80) => {
                Self::ResetPinRetries(ResetPinRetries::try_from(data)?)
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

            _ => return Err(Status::FunctionNotSupported),
        })
    }
}
