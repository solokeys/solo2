use heapless_bytes::Bytes;
use trussed::{
    block,
    Client as TrussedClient,
    syscall,
    types::{ObjectHandle, PathBuf, StorageLocation},
};

use crate::constants::*;

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
    //
    // TODO: it is really not good to overwrite user data on failure to decode old state.
    // To fix this, need a flag to detect if we're "fresh", and/or initialize state in factory.
    pub fn persistent<T: TrussedClient>(&mut self, trussed: &mut T) -> &mut Persistent {
        if self.persistent.is_none() {
            self.persistent = Some(match Persistent::load(trussed) {
                Ok(previous_self) => {
                    // hprintln!("loading succeeded!\n{:?}", &previous_self).ok();
                    previous_self
                }
                Err(_) => {

                    // hprintln!("loading failed: {:?}", e).ok();
                    Persistent::initialize(trussed)
                }
            });
        }
        self.persistent.as_mut().unwrap()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Pin {
    // padded_pin: [u8; 8]
    padded_pin: heapless_bytes::Bytes<heapless::consts::U8>,
}

// impl Default for Pin {
//     /// Default is "202020"
//     /// But right now we have to use "123456" cause.. Filo
//     fn default() -> Self {
//         // Self::try_new(b"202020\xff\xff").unwrap()
//         Self::try_new(b"123456\xff\xff").unwrap()
//     }
// }

impl Pin {
    pub fn try_new(padded_pin: &[u8]) -> Result<Self> {
        if padded_pin.len() != 8 {
            return Err(());
        }
        let first_pad_byte = padded_pin.iter().position(|&b| b == 0xff);
        let unpadded_pin = match first_pad_byte {
            Some(l) => &padded_pin[..l],
            None => padded_pin,
        };
        if unpadded_pin.len() < 6 {
            return Err(());
        }
        let valid_bytes = unpadded_pin.iter().all(|&b| b >= b'0' && b <= b'9');
        if valid_bytes {
            Ok(Self {
                // padded_pin: padded_pin.try_into().unwrap(),
                padded_pin: Bytes::try_from_slice(padded_pin).unwrap(),//padded_pin.try_into().unwrap(),
            })
        } else {
            Err(())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Persistent {
    pub keys: Keys,
    consecutive_pin_mismatches: u8,
    consecutive_puk_mismatches: u8,
    // the PIN can be 6-8 digits, padded with 0xFF if <8
    // we just store all of them for now.
    pin: Pin,
    // the PUK should be 8 digits, but it seems Yubico allows 6-8
    // like for PIN
    puk: Pin,
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
    pub chained_command: Option<iso7816::Command>,
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
    pub pin_verified: bool,
    pub puk_verified: bool,
    pub management_verified: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandCache {
    GetData(GetData),
    AuthenticateManagement(AuthenticateManagement),
}


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetData {
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticateManagement {
    pub challenge: [u8; 8],
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Keys {
    // 9a "PIV Authentication Key" (YK: PIV Authentication)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_key: Option<ObjectHandle>,
    // 9b "PIV Card Application Administration Key" (YK: PIV Management)
    pub management_key: ObjectHandle,
    // 9c "Digital Signature Key" (YK: Digital Signature)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_key: Option<ObjectHandle>,
    // 9d "Key Management Key" (YK: Key Management)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption_key: Option<ObjectHandle>,
    // 9e "Card Authentication Key" (YK: Card Authentication)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinless_authentication_key: Option<ObjectHandle>,
}

impl Persistent
{
    pub const PIN_RETRIES_DEFAULT: u8 = 3;
    // hmm...!
    pub const PUK_RETRIES_DEFAULT: u8 = 5;
    const FILENAME: &'static [u8] = b"persistent-state.cbor";
    const DEFAULT_PIN: &'static [u8] = b"123456\xff\xff";
    const DEFAULT_PUK: &'static [u8] = b"12345678";

    pub fn remaining_pin_retries(&self) -> u8 {
        if self.consecutive_pin_mismatches >= Self::PIN_RETRIES_DEFAULT {
            0
        } else {
            Self::PIN_RETRIES_DEFAULT - self.consecutive_pin_mismatches
        }
    }

    pub fn remaining_puk_retries(&self) -> u8 {
        if self.consecutive_puk_mismatches >= Self::PUK_RETRIES_DEFAULT {
            0
        } else {
            Self::PUK_RETRIES_DEFAULT - self.consecutive_puk_mismatches
        }
    }

    pub fn verify_pin(&self, other_pin: &Pin) -> bool {
        // hprintln!("verifying pin {:?} against {:?}", other_pin, &self.pin).ok();
        self.pin == *other_pin
    }

    pub fn verify_puk(&self, other_puk: &Pin) -> bool {
        // hprintln!("verifying puk {:?} against {:?}", other_puk, &self.puk).ok();
        self.puk == *other_puk
    }

    pub fn set_pin<T: TrussedClient>(&mut self, trussed: &mut T, new_pin: Pin) {
        self.pin = new_pin;
        self.save(trussed);
    }

    pub fn set_puk<T: TrussedClient>(&mut self, trussed: &mut T, new_puk: Pin) {
        self.puk = new_puk;
        self.save(trussed);
    }

    pub fn reset_pin<T: TrussedClient>(&mut self, trussed: &mut T) {
        self.set_pin(trussed, Pin::try_new(Self::DEFAULT_PIN).unwrap());
        self.reset_consecutive_pin_mismatches(trussed);
    }

    pub fn reset_puk<T: TrussedClient>(&mut self, trussed: &mut T) {
        self.set_puk(trussed, Pin::try_new(Self::DEFAULT_PUK).unwrap());
        self.reset_consecutive_puk_mismatches(trussed);
    }

    pub fn increment_consecutive_pin_mismatches<T: TrussedClient>(&mut self, trussed: &mut T) -> u8 {
        if self.consecutive_pin_mismatches >= Self::PIN_RETRIES_DEFAULT {
            return 0;
        }

        self.consecutive_pin_mismatches += 1;
        self.save(trussed);
        Self::PIN_RETRIES_DEFAULT - self.consecutive_pin_mismatches
    }

    pub fn increment_consecutive_puk_mismatches<T: TrussedClient>(&mut self, trussed: &mut T) -> u8 {
        if self.consecutive_puk_mismatches >= Self::PUK_RETRIES_DEFAULT {
            return 0;
        }

        self.consecutive_puk_mismatches += 1;
        self.save(trussed);
        Self::PUK_RETRIES_DEFAULT - self.consecutive_puk_mismatches
    }

    pub fn reset_consecutive_pin_mismatches<T: TrussedClient>(&mut self, trussed: &mut T) -> u8 {
        if self.consecutive_pin_mismatches != 0 {
            self.consecutive_pin_mismatches = 0;
            self.save(trussed);
        }

        Self::PIN_RETRIES_DEFAULT
    }

    pub fn reset_consecutive_puk_mismatches<T: TrussedClient>(&mut self, trussed: &mut T) -> u8 {
        if self.consecutive_puk_mismatches != 0 {
            self.consecutive_puk_mismatches = 0;
            self.save(trussed);
        }

        Self::PUK_RETRIES_DEFAULT
    }

    pub fn reset_management_key<T: TrussedClient>(&mut self, trussed: &mut T) {
        self.set_management_key(trussed, YUBICO_DEFAULT_MANAGEMENT_KEY);
    }

    pub fn set_management_key<T: TrussedClient>(&mut self, trussed: &mut T, management_key: &[u8; 24]) {
        let new_management_key = syscall!(trussed.unsafe_inject_tdes_key(
            management_key,
            trussed::types::StorageLocation::Internal,
        )).key;
        let old_management_key = self.keys.management_key;
        self.keys.management_key = new_management_key;
        self.save(trussed);
        syscall!(trussed.delete(old_management_key));
    }

    pub fn initialize<T: TrussedClient>(trussed: &mut T) -> Self {
        let management_key = syscall!(trussed.unsafe_inject_tdes_key(
            YUBICO_DEFAULT_MANAGEMENT_KEY,
            trussed::types::StorageLocation::Internal,
        )).key;

        let keys = Keys {
            authentication_key: None,
            management_key: management_key,
            signature_key: None,
            encryption_key: None,
            pinless_authentication_key: None,
        };

        Self {
            keys,
            consecutive_pin_mismatches: 0,
            consecutive_puk_mismatches: 0,
            pin: Pin::try_new(Self::DEFAULT_PIN).unwrap(),
            puk: Pin::try_new(Self::DEFAULT_PUK).unwrap(),
            timestamp: 0,
        }
    }

    pub fn load<T: TrussedClient>(trussed: &mut T) -> Result<Self> {
        let data = block!(trussed.read_file(
                StorageLocation::Internal,
                PathBuf::from(Self::FILENAME),
            ).unwrap()
        ).map_err(|e| {
            // hprintln!("loading error: {:?}", &e).ok();
            drop(e)
        })?.data;

        let previous_state: Persistent = trussed::cbor_deserialize(&data).map_err(|e| {
            // hprintln!("cbor deser error: {:?}", e);
            // hprintln!("data: {:X?}", &data).ok();
            drop(e)
        })?;
        // horrible deser bug to forget Ok here :)
        Ok(previous_state)
    }

    pub fn save<T: TrussedClient>(&self, trussed: &mut T) {
        let data: trussed::types::Message = trussed::cbor_serialize_bytebuf(self).unwrap();

        syscall!(trussed.write_file(
            StorageLocation::Internal,
            PathBuf::from(Self::FILENAME),
            data,
            None,
        ));
    }

    pub fn timestamp<T: TrussedClient>(&mut self, trussed: &mut T) -> u32 {
        self.timestamp += 1;
        self.save(trussed);
        self.timestamp
    }

}

impl Runtime {
}

