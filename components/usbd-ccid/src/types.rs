// use cortex_m_semihosting::hprintln;
use heapless_bytes::Bytes;

use crate::constants::*;
use core::time::Duration;

// pub mod apdu;
pub mod packet;
pub mod tlv;

pub type MessageBuffer = Bytes<MAX_MSG_LENGTH_TYPE>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ClassRequest {
    Abort = 1,
    GetClockFrequencies = 2,
    GetDataRates = 3,
}

pub enum Status {
    Idle,
    ReceivedData(Duration),
}

impl core::convert::TryFrom<u8> for ClassRequest {
    type Error = ();
    fn try_from(request: u8) -> core::result::Result<Self, ()> {
        Ok(match request {
            1 => Self::Abort,
            2 => Self::GetClockFrequencies,
            3 => Self::GetDataRates,
            _ => return Err(()),
        })
    }
}

