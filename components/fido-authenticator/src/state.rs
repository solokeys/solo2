use core::cmp::Ordering;

use cortex_m_semihosting::hprintln;

use trussed::{
    Client as CryptoClient,
    pipe::Syscall,
    types::{
        self,
        ObjectHandle as Key,
        StorageLocation,
    },
};
use ctap_types::{
    ByteBuf, ByteBuf32, consts,
    authenticator::Error,
    cose::EcdhEsHkdf256PublicKey as CoseEcdhEsHkdf256PublicKey,
    sizes::MAX_CREDENTIAL_COUNT_IN_LIST, // U8 currently
};

use heapless::binary_heap::{BinaryHeap, Max, Min};
use littlefs2::path::PathBuf;
use ufmt::derive::uDebug;

use crate::Result;
use crate::cbor_serialize_message;

pub type MaxCredentialHeap = BinaryHeap<TimestampPath, MAX_CREDENTIAL_COUNT_IN_LIST, Max>;
pub type MinCredentialHeap = BinaryHeap<TimestampPath, MAX_CREDENTIAL_COUNT_IN_LIST, Min>;

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

#[derive(Clone, Debug, /*uDebug, Eq, PartialEq,*/ serde::Deserialize, serde::Serialize)]
pub struct State {
    pub identity: Identity,
    pub persistent: PersistentState,
    pub runtime: RuntimeState,
}

impl State {

    // pub fn new<S: Syscall>(crypto: &mut CryptoClient<S>) -> Self {
    pub fn new() -> Self {
        // let identity = Identity::get(crypto);
        let identity = Default::default();
        let runtime: RuntimeState = Default::default();
        // let persistent = PersistentState::load_or_reset(crypto);
        let persistent = Default::default();

        Self { identity, persistent, runtime }
    }

    pub fn decrement_retries<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<()> {
        self.persistent.decrement_retries(crypto)?;
        self.runtime.decrement_retries();
        Ok(())
    }

    pub fn reset_retries<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<()> {
        self.persistent.reset_retries(crypto)?;
        self.runtime.reset_retries();
        Ok(())
    }


    pub fn pin_blocked(&self) -> Result<()> {

        if self.persistent.pin_blocked() {
            return Err(Error::PinBlocked);
        }
        if self.runtime.pin_blocked() {
            return Err(Error::PinAuthBlocked);
        }

        Ok(())
    }

}

#[derive(Clone, Debug, uDebug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Identity {
    // can this be [u8; 16] or need ByteBuf for serialization?
    // aaguid: Option<ByteBuf<consts::U16>>,
    attestation_key: Option<Key>,
}

impl Identity {
    // pub fn get<S: Syscall>(crypto: &mut CryptoClient<S>) -> Self {

    //     // TODO: inject properly
    //     let attestation_key = syscall!(crypto
    //         .generate_p256_private_key(StorageLocation::Internal))
    //         .key;

    //     Self {
    //         aaguid: ByteBuf::from_slice(b"AAGUID0123456789").unwrap(),
    //         attestation_key,
    //     }
    // }

    pub fn aaguid(&self) -> ByteBuf<consts::U16> {
        ByteBuf::from_slice(b"AAGUID0123456789").unwrap()
    }

    pub fn attestation_key<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Key
    {
        match self.attestation_key {
            Some(key) => key,
            None => self.load_attestation_key(crypto),
        }
    }

    fn load_attestation_key<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Key {
        let key = syscall!(crypto
            .generate_p256_private_key(StorageLocation::Internal))
            .key;
        self.attestation_key = Some(key);
        key
    }

}

#[derive(Clone, Debug, uDebug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum CommandCache {
    CredentialManagementEnumerateRps(u32, ByteBuf32),
    CredentialManagementEnumerateCredentials(u32, PathBuf, PathBuf),
}

#[derive(Clone, Debug, /*uDebug,*/ Default, /*PartialEq,*/ serde::Deserialize, serde::Serialize)]
pub struct ActiveGetAssertionData {
    pub rp_id_hash: [u8; 32],
    pub client_data_hash: [u8; 32],
    pub uv_performed: bool,
}

#[derive(Clone, Debug, /*uDebug,*/ Default, /*PartialEq,*/ serde::Deserialize, serde::Serialize)]
pub struct RuntimeState {
    key_agreement_key: Option<Key>,
    pin_token: Option<Key>,
    // TODO: why is this field not used?
    shared_secret: Option<Key>,
    consecutive_pin_mismatches: u8,

    // both of these are a cache for previous Get{Next,}Assertion call
    credentials: Option<MaxCredentialHeap>,
    pub active_get_assertion: Option<ActiveGetAssertionData>,
    channel: Option<u32>,
    pub cache: Option<CommandCache>,
}

// TODO: Plan towards future extensibility
//
// - if we set all fields as optional, and annotate with `skip_serializing if None`,
// then, missing fields in older fw versions should not cause problems with newer fw
// versions that potentially add new fields.
//
// - empirically, the implementation of Deserialize doesn't seem to mind moving around
// the order of fields, which is already nice
//
// - adding new non-optional fields definitely doesn't parse (but maybe it could?)
// - same for removing a field
// Currently, this causes the entire authnr to reset state. Maybe it should even reformat disk
//
// - An alternative would be `heapless::Map`, but I'd prefer something more typed.
#[derive(Clone, Debug, uDebug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PersistentState {
    #[serde(skip)]
    // TODO: there has to be a better way than.. this
    // Pro-tip: it should involve types ^^
    //
    // We could alternatively make all methods take a CryptoClient as parameter
    initialised: bool,

    key_encryption_key: Option<Key>,
    key_wrapping_key: Option<Key>,
    consecutive_pin_mismatches: u8,
    pin_hash: Option<[u8; 16]>,
    // Ideally, we'd dogfood a "Monotonic Counter" from trussed.
    // TODO: Add per-key counters for resident keys.
    // counter: Option<Key>,
    timestamp: u32,
}

impl PersistentState {

    const RESET_RETRIES: u8 = 8;
    const FILENAME: &'static [u8] = b"persistent-state.cbor";
    const MAX_RESIDENT_CREDENTIALS_GUESSTIMATE: u32 = 100;

    pub fn max_resident_credentials_guesstimate(&self) -> u32 {
        Self::MAX_RESIDENT_CREDENTIALS_GUESSTIMATE
    }

    pub fn load<S: Syscall>(crypto: &mut CryptoClient<S>) -> Result<Self> {

        // TODO: add "exists_file" method instead?
        let data = block!(crypto.read_file(
                StorageLocation::Internal,
                PathBuf::from(Self::FILENAME),
            ).unwrap()
        ).map_err(|_| Error::Other)?.data;

        let previous_state = trussed::cbor_deserialize(&data).map_err(|_| Error::Other);
        cortex_m_semihosting::hprintln!("previously persisted state:\n{:?}", &previous_state).ok();
        previous_state
    }

    pub fn save<S: Syscall>(&self, crypto: &mut CryptoClient<S>) -> Result<()> {
        let data = crate::cbor_serialize_message(self).unwrap();

        syscall!(crypto.write_file(
            StorageLocation::Internal,
            PathBuf::from(Self::FILENAME),
            data,
            None,
        ));
        Ok(())
    }

    pub fn reset<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<()> {
        self.key_encryption_key = None;
        self.key_wrapping_key = None;
        self.consecutive_pin_mismatches = 0;
        self.pin_hash = None;
        self.timestamp = 0;
        self.save(crypto)
    }

    pub fn load_if_not_initialised<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) {
        if !self.initialised {
            if let Ok(previous_self) = Self::load(crypto) {
                *self = previous_self
            }
            self.initialised = true;
        }
    }

    pub fn timestamp<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<u32> {
        self.load_if_not_initialised(crypto);

        let now = self.timestamp;
        self.timestamp += 1;
        self.save(crypto)?;
        // cortex_m_semihosting::hprintln!("https://time.is/{}", now).ok();
        Ok(now)
    }

    pub fn key_encryption_key<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<Key>
    {
        self.load_if_not_initialised(crypto);
        match self.key_encryption_key {
            Some(key) => Ok(key),
            None => self.rotate_key_encryption_key(crypto),
        }
    }

    pub fn rotate_key_encryption_key<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<Key> {
        self.load_if_not_initialised(crypto);
        if let Some(key) = self.key_encryption_key { syscall!(crypto.delete(key)); }
        let key = syscall!(crypto.generate_chacha8poly1305_key(StorageLocation::Internal)).key;
        self.key_encryption_key = Some(key);
        self.save(crypto)?;
        Ok(key)
    }

    pub fn key_wrapping_key<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<Key>
    {
        self.load_if_not_initialised(crypto);
        match self.key_wrapping_key {
            Some(key) => Ok(key),
            None => self.rotate_key_wrapping_key(crypto),
        }
    }

    pub fn rotate_key_wrapping_key<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<Key> {
        self.load_if_not_initialised(crypto);
        if let Some(key) = self.key_wrapping_key { syscall!(crypto.delete(key)); }
        let key = syscall!(crypto.generate_chacha8poly1305_key(StorageLocation::Internal)).key;
        self.key_wrapping_key = Some(key);
        self.save(crypto)?;
        Ok(key)
    }

    pub fn pin_is_set(&self) -> bool {
        self.pin_hash.is_some()
    }

    pub fn retries(&self) -> u8 {
        Self::RESET_RETRIES - self.consecutive_pin_mismatches
    }

    pub fn pin_blocked(&self) -> bool {
        self.consecutive_pin_mismatches >= Self::RESET_RETRIES
    }

     fn reset_retries<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<()> {
        self.load_if_not_initialised(crypto);
        if self.consecutive_pin_mismatches > 0 {
            self.consecutive_pin_mismatches = 0;
            self.save(crypto)?;
        }
        Ok(())
    }

    fn decrement_retries<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Result<()> {
        self.load_if_not_initialised(crypto);
        // error to call before initialization
        if self.consecutive_pin_mismatches < Self::RESET_RETRIES {
            self.consecutive_pin_mismatches += 1;
            self.save(crypto)?;
        }
        Ok(())
    }

    pub fn pin_hash(&self) -> Option<[u8; 16]> {
        self.pin_hash
    }

    pub fn set_pin_hash<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>, pin_hash: [u8; 16]) -> Result<()> {
        self.load_if_not_initialised(crypto);
        self.pin_hash = Some(pin_hash);
        self.save(crypto)?;
        Ok(())
    }


}

impl RuntimeState {

    const POWERCYCLE_RETRIES: u8 = 3;

    fn decrement_retries(&mut self) {
        if self.consecutive_pin_mismatches < Self::POWERCYCLE_RETRIES {
            self.consecutive_pin_mismatches += 1;
        }
    }

    fn reset_retries(&mut self) {
        self.consecutive_pin_mismatches = 0;
    }


    pub fn pin_blocked(&self) -> bool {
        self.consecutive_pin_mismatches >= Self::POWERCYCLE_RETRIES
    }

    pub fn credential_heap(&mut self) -> &mut MaxCredentialHeap {
        if self.credentials.is_none() {
            self.create_credential_heap()
        } else {
            self.credentials.as_mut().unwrap()
        }
    }

    fn create_credential_heap(&mut self) -> &mut MaxCredentialHeap {
        self.credentials = Some(MaxCredentialHeap::new());
        self.credentials.as_mut().unwrap()
    }

    pub fn key_agreement_key<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Key {
        match self.key_agreement_key {
            Some(key) => key,
            None => self.rotate_key_agreement_key(crypto),
        }
    }

    pub fn rotate_key_agreement_key<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Key {
        // TODO: need to rotate pin token?
        if let Some(key) = self.key_agreement_key { syscall!(crypto.delete(key)); }

        let key = syscall!(crypto.generate_p256_private_key(StorageLocation::Volatile)).key;
        self.key_agreement_key = Some(key);
        key
    }

    pub fn pin_token<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Key {
        match self.pin_token {
            Some(token) => token,
            None => self.rotate_pin_token(crypto),
        }
    }

    pub fn rotate_pin_token<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) -> Key {
        // TODO: need to rotate key agreement key?
        hprintln!("rotatating pin token!!").ok();
        if let Some(token) = self.pin_token { syscall!(crypto.delete(token)); }
        let token = syscall!(crypto.generate_hmacsha256_key(StorageLocation::Volatile)).key;
        self.pin_token = Some(token);
        token
    }

    pub fn reset<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>) {
        self.rotate_key_agreement_key(crypto);
        self.rotate_pin_token(crypto);
        // self.drop_shared_secret(crypto);
        self.credentials = None;
        self.active_get_assertion = None;
    }

    // TODO: don't recalculate constantly
    pub fn generate_shared_secret<S: Syscall>(&mut self, crypto: &mut CryptoClient<S>, platform_key_agreement_key: &CoseEcdhEsHkdf256PublicKey) -> Result<Key> {
        let private_key = self.key_agreement_key(crypto);

        let serialized_pkak = cbor_serialize_message(platform_key_agreement_key).map_err(|_| Error::InvalidParameter)?;
        let platform_kak = block!(
            crypto.deserialize_key(
                types::Mechanism::P256, serialized_pkak, types::KeySerialization::EcdhEsHkdf256,
                types::StorageAttributes::new().set_persistence(types::StorageLocation::Volatile)
            ).unwrap()).map_err(|_| Error::InvalidParameter)?.key;

        let pre_shared_secret = syscall!(crypto.agree(
            types::Mechanism::P256, private_key.clone(), platform_kak.clone(),
            types::StorageAttributes::new().set_persistence(types::StorageLocation::Volatile),
        )).shared_secret;
        syscall!(crypto.delete(platform_kak));

        if let Some(previous_shared_secret) = self.shared_secret {
            syscall!(crypto.delete(previous_shared_secret));
        }

        let shared_secret = syscall!(crypto.derive_key(
            types::Mechanism::Sha256, pre_shared_secret.clone(), types::StorageAttributes::new().set_persistence(types::StorageLocation::Volatile)
        )).key;
        self.shared_secret = Some(shared_secret);

        syscall!(crypto.delete(pre_shared_secret));

        Ok(shared_secret)
    }

}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TimestampPath {
    pub timestamp: u32,
    pub path: PathBuf,
}

impl Ord for TimestampPath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl PartialOrd for TimestampPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

