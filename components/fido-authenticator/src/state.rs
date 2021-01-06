use core::cmp::Ordering;

use trussed::{
    block, syscall,
    Client as TrussedClient,
    types::{
        self,
        ObjectHandle as Key,
        UniqueId,
        StorageLocation,
        Mechanism,
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

use crate::Result;
use crate::cbor_serialize_message;

pub type MaxCredentialHeap = BinaryHeap<TimestampPath, MAX_CREDENTIAL_COUNT_IN_LIST, Max>;
pub type MinCredentialHeap = BinaryHeap<TimestampPath, MAX_CREDENTIAL_COUNT_IN_LIST, Min>;

#[derive(Clone, Debug, /*uDebug, Eq, PartialEq,*/ serde::Deserialize, serde::Serialize)]
pub struct State {
    pub identity: Identity,
    pub persistent: PersistentState,
    pub runtime: RuntimeState,
}

impl State {

    // pub fn new(trussed: &mut TrussedClient) -> Self {
    pub fn new() -> Self {
        // let identity = Identity::get(trussed);
        let identity = Default::default();
        let runtime: RuntimeState = Default::default();
        // let persistent = PersistentState::load_or_reset(trussed);
        let persistent = Default::default();

        Self { identity, persistent, runtime }
    }

    pub fn decrement_retries<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<()> {
        self.persistent.decrement_retries(trussed)?;
        self.runtime.decrement_retries()?;
        Ok(())
    }

    pub fn reset_retries<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<()> {
        self.persistent.reset_retries(trussed)?;
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

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Identity {
    // can this be [u8; 16] or need ByteBuf for serialization?
    // aaguid: Option<ByteBuf<consts::U16>>,
    attestation_key: Option<Key>,
}

impl Identity {
    // pub fn get(trussed: &mut TrussedClient) -> Self {

    //     // TODO: inject properly
    //     let attestation_key = syscall!(trussed
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

    pub fn attestation_key<T: TrussedClient>(&mut self, trussed: &mut T) -> Option<Key>
    {
        let key = Key {
            object_id: UniqueId::from(0)
        };
        let attestation_key_exists = syscall!(trussed.exists(Mechanism::P256, key)).exists;
        if attestation_key_exists {
            Some(key)
        } else {
            None
        }
    }

}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum CommandCache {
    CredentialManagementEnumerateRps(u32, ByteBuf32),
    CredentialManagementEnumerateCredentials(u32, PathBuf, PathBuf),
}

#[derive(Clone, Debug, /*uDebug,*/ Default, /*PartialEq,*/ serde::Deserialize, serde::Serialize)]
pub struct ActiveGetAssertionData {
    pub rp_id_hash: [u8; 32],
    pub client_data_hash: [u8; 32],
    pub uv_performed: bool,
    pub up_performed: bool,
    pub multiple_credentials: bool,
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
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PersistentState {
    #[serde(skip)]
    // TODO: there has to be a better way than.. this
    // Pro-tip: it should involve types ^^
    //
    // We could alternatively make all methods take a TrussedClient as parameter
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

    pub fn load<T: TrussedClient>(trussed: &mut T) -> Result<Self> {

        // TODO: add "exists_file" method instead?
        let result = block!(trussed.read_file(
                StorageLocation::Internal,
                PathBuf::from(Self::FILENAME),
            ).unwrap()
        ).map_err(|_| Error::Other);

        if result.is_err() {
            info_now!("err loading: {:?}", result.err().unwrap());
            return Err(Error::Other);
        }

        let data = result.unwrap().data;

        let result = trussed::cbor_deserialize(&data);

        if result.is_err() {
            info_now!("err deser'ing: {:?}", result.err().unwrap());
            info_now!("{}", hex_str!(&data));
            return Err(Error::Other);
        }

        let previous_state = result.map_err(|_| Error::Other);

        previous_state
    }

    pub fn save<T: TrussedClient>(&self, trussed: &mut T) -> Result<()> {
        let data = crate::cbor_serialize_message(self).unwrap();

        syscall!(trussed.write_file(
            StorageLocation::Internal,
            PathBuf::from(Self::FILENAME),
            data,
            None,
        ));
        Ok(())
    }

    pub fn reset<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<()> {
        if let Some(key) = self.key_encryption_key {
            syscall!(trussed.delete(key));
        }
        if let Some(key) = self.key_wrapping_key {
            syscall!(trussed.delete(key));
        }
        self.key_encryption_key = None;
        self.key_wrapping_key = None;
        self.consecutive_pin_mismatches = 0;
        self.pin_hash = None;
        self.timestamp = 0;
        self.save(trussed)
    }

    pub fn load_if_not_initialised<T: TrussedClient>(&mut self, trussed: &mut T) {
        if !self.initialised {
            match Self::load(trussed) {
                Ok(previous_self) => {
                    info!("loaded previous state!");
                    *self = previous_self
                },
                Err(_err) => {
                    info!("error with previous state! {:?}", _err);
                }
            }
            self.initialised = true;
        }
    }

    pub fn timestamp<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<u32> {
        let now = self.timestamp;
        self.timestamp += 1;
        self.save(trussed)?;
        Ok(now)
    }

    pub fn key_encryption_key<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<Key>
    {
        match self.key_encryption_key {
            Some(key) => Ok(key),
            None => self.rotate_key_encryption_key(trussed),
        }
    }

    pub fn rotate_key_encryption_key<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<Key> {
        if let Some(key) = self.key_encryption_key { syscall!(trussed.delete(key)); }
        let key = syscall!(trussed.generate_chacha8poly1305_key(StorageLocation::Internal)).key;
        self.key_encryption_key = Some(key);
        self.save(trussed)?;
        Ok(key)
    }

    pub fn key_wrapping_key<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<Key>
    {
        match self.key_wrapping_key {
            Some(key) => Ok(key),
            None => self.rotate_key_wrapping_key(trussed),
        }
    }

    pub fn rotate_key_wrapping_key<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<Key> {
        self.load_if_not_initialised(trussed);
        if let Some(key) = self.key_wrapping_key { syscall!(trussed.delete(key)); }
        let key = syscall!(trussed.generate_chacha8poly1305_key(StorageLocation::Internal)).key;
        self.key_wrapping_key = Some(key);
        self.save(trussed)?;
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

     fn reset_retries<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<()> {
        if self.consecutive_pin_mismatches > 0 {
            self.consecutive_pin_mismatches = 0;
            self.save(trussed)?;
        }
        Ok(())
    }

    fn decrement_retries<T: TrussedClient>(&mut self, trussed: &mut T) -> Result<()> {
        // error to call before initialization
        if self.consecutive_pin_mismatches < Self::RESET_RETRIES {
            self.consecutive_pin_mismatches += 1;
            self.save(trussed)?;
            if self.consecutive_pin_mismatches == 0 {
                return Err(Error::PinBlocked);
            }
        }
        Ok(())
    }

    pub fn pin_hash(&self) -> Option<[u8; 16]> {
        self.pin_hash
    }

    pub fn set_pin_hash<T: TrussedClient>(&mut self, trussed: &mut T, pin_hash: [u8; 16]) -> Result<()> {
        self.pin_hash = Some(pin_hash);
        self.save(trussed)?;
        Ok(())
    }


}

impl RuntimeState {

    const POWERCYCLE_RETRIES: u8 = 3;

    fn decrement_retries(&mut self) -> Result<()> {
        if self.consecutive_pin_mismatches < Self::POWERCYCLE_RETRIES {
            self.consecutive_pin_mismatches += 1;
        }
        if self.consecutive_pin_mismatches == Self::POWERCYCLE_RETRIES {
            Err(Error::PinAuthBlocked)
        } else {
            Ok(())
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

    pub fn key_agreement_key<T: TrussedClient>(&mut self, trussed: &mut T) -> Key {
        match self.key_agreement_key {
            Some(key) => key,
            None => self.rotate_key_agreement_key(trussed),
        }
    }

    pub fn rotate_key_agreement_key<T: TrussedClient>(&mut self, trussed: &mut T) -> Key {
        // TODO: need to rotate pin token?
        if let Some(key) = self.key_agreement_key { syscall!(trussed.delete(key)); }

        let key = syscall!(trussed.generate_p256_private_key(StorageLocation::Volatile)).key;
        self.key_agreement_key = Some(key);
        key
    }

    pub fn pin_token<T: TrussedClient>(&mut self, trussed: &mut T) -> Key {
        match self.pin_token {
            Some(token) => token,
            None => self.rotate_pin_token(trussed),
        }
    }

    pub fn rotate_pin_token<T: TrussedClient>(&mut self, trussed: &mut T) -> Key {
        // TODO: need to rotate key agreement key?
        if let Some(token) = self.pin_token { syscall!(trussed.delete(token)); }
        let token = syscall!(trussed.generate_hmacsha256_key(StorageLocation::Volatile)).key;
        self.pin_token = Some(token);
        token
    }

    pub fn reset<T: TrussedClient>(&mut self, trussed: &mut T) {
        self.rotate_key_agreement_key(trussed);
        self.rotate_pin_token(trussed);
        // self.drop_shared_secret(trussed);
        self.credentials = None;
        self.active_get_assertion = None;
    }

    // TODO: don't recalculate constantly
    pub fn generate_shared_secret<T: TrussedClient>(&mut self, trussed: &mut T, platform_key_agreement_key: &CoseEcdhEsHkdf256PublicKey) -> Result<Key> {
        let private_key = self.key_agreement_key(trussed);

        let serialized_pkak = cbor_serialize_message(platform_key_agreement_key).map_err(|_| Error::InvalidParameter)?;
        let platform_kak = block!(
            trussed.deserialize_key(
                types::Mechanism::P256, serialized_pkak, types::KeySerialization::EcdhEsHkdf256,
                types::StorageAttributes::new().set_persistence(types::StorageLocation::Volatile)
            ).unwrap()).map_err(|_| Error::InvalidParameter)?.key;

        let pre_shared_secret = syscall!(trussed.agree(
            types::Mechanism::P256, private_key.clone(), platform_kak.clone(),
            types::StorageAttributes::new().set_persistence(types::StorageLocation::Volatile),
        )).shared_secret;
        syscall!(trussed.delete(platform_kak));

        if let Some(previous_shared_secret) = self.shared_secret {
            syscall!(trussed.delete(previous_shared_secret));
        }

        let shared_secret = syscall!(trussed.derive_key(
            types::Mechanism::Sha256, pre_shared_secret.clone(), types::StorageAttributes::new().set_persistence(types::StorageLocation::Volatile)
        )).key;
        self.shared_secret = Some(shared_secret);

        syscall!(trussed.delete(pre_shared_secret));

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

