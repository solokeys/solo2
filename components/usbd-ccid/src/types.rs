use embedded_time::duration::Milliseconds;

// pub mod apdu;
pub mod packet;
pub mod tlv;

pub type MessageBuffer = apdu_dispatch::interchanges::Data;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ClassRequest {
    Abort = 1,
    GetClockFrequencies = 2,
    GetDataRates = 3,
}

pub enum Status {
    Idle,
    ReceivedData(Milliseconds),
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

