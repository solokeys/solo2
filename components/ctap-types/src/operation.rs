use core::convert::TryFrom;

/// the authenticator API, consisting of "operations"
#[derive(Copy,Clone,Debug,uDebug,Eq,PartialEq)]
pub enum Operation {
    MakeCredential,
    GetAssertion,
    GetNextAssertion,
    GetInfo,
    ClientPin,
    Reset,
    // new in v2.1
    BioEnrollment,
    // new in v2.1
    CredentialManagement,
    /// vendors are assigned the range 0x40..=0x7f for custom operations
    Vendor(VendorOperation),
}

impl Into<u8> for Operation {
    fn into(self) -> u8 {
        match self {
            Operation::MakeCredential => 0x01,
            Operation::GetAssertion => 0x02,
            Operation::GetNextAssertion => 0x08,
            Operation::GetInfo => 0x04,
            Operation::ClientPin => 0x06,
            Operation::Reset => 0x07,
            Operation::BioEnrollment => 0x09,
            Operation::CredentialManagement => 0x0A,
            Operation::Vendor(operation) => operation.into(),
        }
    }
}

impl Operation {
    pub fn into_u8(self) -> u8 {
        self.into()
    }
}

/// Vendor CTAP2 operations, from 0x40 to 0x7f.
#[derive(Copy,Clone,Debug,uDebug,Eq,PartialEq)]
pub struct VendorOperation(u8);

impl VendorOperation {
    pub const FIRST: u8 = 0x40;
    pub const LAST: u8 = 0x7f;
}

impl TryFrom<u8> for VendorOperation {
    type Error = ();

    fn try_from(from: u8) -> core::result::Result<Self, ()> {
        match from {
            // code if code >= Self::FIRST && code <= Self::LAST => Ok(VendorOperation(code)),
            code @ Self::FIRST..=Self::LAST => Ok(VendorOperation(code)),
            _ => Err(()),
        }
    }
}

impl Into<u8> for VendorOperation {
    fn into(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Operation {
    type Error = ();

    fn try_from(from: u8) -> core::result::Result<Operation, ()> {
        match from {
            0x01 => Ok(Operation::MakeCredential),
            0x02 => Ok(Operation::GetAssertion),
            0x08 => Ok(Operation::GetNextAssertion),
            0x04 => Ok(Operation::GetInfo),
            0x06 => Ok(Operation::ClientPin),
            0x07 => Ok(Operation::Reset),
            0x09 => Ok(Operation::BioEnrollment),
            0x0A => Ok(Operation::CredentialManagement),
            code @ VendorOperation::FIRST..=VendorOperation::LAST
                 => Ok(Operation::Vendor(VendorOperation::try_from(code)?)),
            _ => Err(()),
        }
    }
}

