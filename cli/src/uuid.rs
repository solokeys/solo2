use crate::{Error, Result};

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Uuid(u128);

impl From<u128> for Uuid {
    fn from(number: u128) -> Self {
        Self(number)
    }
}

impl From<[u8; 16]> for Uuid {
    fn from(hex: [u8; 16]) -> Self {
        Self(u128::from_be_bytes(hex))
    }
}

impl TryFrom<&[u8]> for Uuid {
    type Error = Error;
    fn try_from(slice: &[u8]) -> Result<Self> {
        let array: [u8; 16] = slice.try_into()?;
        Ok(array.into())
    }
}

impl Uuid {
    pub fn bytes(&self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    pub fn hex(&self) -> String {
        hex::encode_upper(self.bytes())
    }

    pub fn u128(&self) -> u128 {
        self.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        bytes.try_into()
    }

    pub fn from_hex(hex: &str) -> Result<Self> {
        let bytes = hex::decode(hex)?;
        bytes.as_slice().try_into()
    }
}
