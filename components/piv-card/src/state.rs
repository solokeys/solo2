use trussed::{
    Client as Trussed,
    pipe::Syscall,
    types::{PathBuf, StorageLocation},
};

use crate::constants::*;

#[macro_use]
macro_rules! block {
    ($future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $future_result;
        loop {
            match future_result.poll() {
                core::task::Poll::Ready(result) => { break result; },
                core::task::Poll::Pending => {},
            }
        }
    }}
}

#[macro_use]
macro_rules! syscall {
    ($pre_future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $pre_future_result.expect("no client error");
        loop {
            match future_result.poll() {
                // core::task::Poll::Ready(result) => { break result.expect("no errors"); },
                core::task::Poll::Ready(result) => { break result.unwrap(); },
                core::task::Poll::Pending => {},
            }
        }
    }}
}

pub type Result<T> = core::result::Result<T, ()>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct State {
    // at startup, trussed is not callable yet.
    // moreover, when worst comes to worst, filesystems are not available
    persistent: Option<Persistent>,
    pub runtime: Runtime,
    // temporary "state", to be removed again
    // pub hack: Hack,
    // trussed: RefCell<Trussed<S>>,
}

impl State {
    pub fn new() -> Self {
        Default::default()
    }

    // it would be nicer to do this during "board bringup", by using TrussedService as Syscall
    pub fn persistent(&mut self, trussed: &mut Trussed) -> &mut Persistent {
        if self.persistent.is_none() {
            self.persistent = Some(match Persistent::load(trussed) {
                Ok(previous_self) => previous_self,
                Err(_) => Default::default(),
            });
        }
        self.persistent.as_mut().unwrap()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Persistent {
    consecutive_pin_mismatches: u8,
    // pin_hash: Option<[u8; 16]>,
    // Ideally, we'd dogfood a "Monotonic Counter" from `trussed`.
    timestamp: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Runtime {
    // aid: Option<
    // consecutive_pin_mismatches: u8,

    pub global_security_status: GlobalSecurityStatus,
    pub currently_selected_application: SelectableAid,
    pub app_security_status: AppSecurityStatus,
    pub command_cache: Option<CommandCache>,
}

pub trait Aid {
    const AID: &'static [u8];
    const RIGHT_TRUNCATED_LENGTH: usize;

    fn len() -> usize {
        Self::AID.len()
    }

    fn full() -> &'static [u8] {
        Self::AID
    }

    fn right_truncated() -> &'static [u8] {
        &Self::AID[..Self::RIGHT_TRUNCATED_LENGTH]
    }

    fn pix() -> &'static [u8] {
        &Self::AID[5..]
    }

    fn rid() -> &'static [u8] {
        &Self::AID[..5]
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SelectableAid {
    Piv(PivAid),
    YubicoOtp(YubicoOtpAid),
}

impl Default for SelectableAid {
    fn default() -> Self {
        Self::Piv(Default::default())
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PivAid {}

impl Aid for PivAid {
    const AID: &'static [u8] = &PIV_AID;
    const RIGHT_TRUNCATED_LENGTH: usize = 9;
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct YubicoOtpAid {}

impl Aid for YubicoOtpAid {
    const AID: &'static [u8] = &YUBICO_OTP_AID;
    const RIGHT_TRUNCATED_LENGTH: usize = 8;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GlobalSecurityStatus {
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppSecurityStatus {
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandCache {
    GetData(GetData),
}


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetData {
}

impl Persistent {
    const PIN_RETRIES_DEFAULT: u8 = 3;
    const FILENAME: &'static [u8] = b"persistent-state.cbor";

    pub fn load(trussed: &mut Trussed) -> Result<Self> {
        let data = block!(trussed.read_file(
                StorageLocation::Internal,
                PathBuf::from(Self::FILENAME),
            ).unwrap()
        ).map_err(drop)?.data;

        let previous_state = trussed::cbor_deserialize(&data).map_err(drop);
        cortex_m_semihosting::hprintln!("previously persisted PIV state:\n{:?}", &previous_state).ok();
        previous_state
    }

    pub fn save(&self, trussed: &mut Trussed) {
        let data: trussed::types::Message = trussed::cbor_serialize_bytebuf(self).unwrap();

        syscall!(trussed.write_file(
            StorageLocation::Internal,
            PathBuf::from(Self::FILENAME),
            data,
            None,
        ));
    }

    pub fn timestamp(&mut self, trussed: &mut Trussed) -> u32 {
        self.timestamp += 1;
        self.save(trussed);
        self.timestamp
    }

}

impl Runtime {
}

