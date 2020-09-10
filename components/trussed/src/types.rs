use core::convert::TryFrom;
use core::marker::PhantomData;

pub use generic_array::GenericArray;

pub use heapless::{
    consts,
    String,
    Vec,
};

pub use heapless::ByteBuf;

pub use littlefs2::{
    fs::{DirEntry, Filesystem},
    driver::Storage as LfsStorage,
    io::Result as LfsResult,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use ufmt::derive::uDebug;

use crate::config::*;

pub use crate::board::Board;
pub use crate::client::FutureResult;

pub mod ui {
    use super::*;

    // TODO: Consider whether a simple "language" to specify "patterns"
    // makes sense, vs. "semantic" indications with board-specific implementation
    #[derive(Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
    pub enum VisualPattern {
        // BreathMajestically,
        BlinkingGreen,
        StaticBlue,
    }
}

pub mod consent {
    use super::*;

    #[derive(Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
    pub enum Level {
        /// Normal user presence check, currently implemented as "touch any of three buttons"
        Normal,
        /// Strong user intent check, currently implemented as "three finger squeeze"
        Strong,
    }

    #[derive(Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
    pub enum Urgency {
        /// Pending other user consent requests will fail as interrupted.
        InterruptOthers,
        /// If other user consent requests are pending, fail this request.
        FailIfOthers,
    }

    #[derive(Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
    pub enum Error {
        FailedToInterrupt,
        Interrupted,
        TimedOut,
    }

    pub type Result = core::result::Result<(), Error>;
}

// for counters use the pkcs#11 idea of
// a monotonic incrementing counter that
// "increments on each read" --> save +=1 operation

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct AeadUniqueId {
    unique_id: [u8; 16],
    nonce: [u8; 12],
    tag: [u8; 16],
}

pub type AeadKey = [u8; 32];
pub type AeadNonce = [u8; 12];
pub type AeadTag = [u8; 16];

// pub type ClientId = heapless::Vec<u8, heapless::consts::U32>;
pub type ClientId = PathBuf;

// Object Hierarchy according to Cryptoki
// - Storage
//   - Domain parameters
//   - Key
//   - Certificate
//   - Data
// - Hardware feature
// - Mechanism
// - Profiles


#[derive(Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
pub enum Attributes {
    Certificate,
    Counter,
    Data(DataAttributes),
    Key(KeyAttributes),
}

#[derive(Clone, Eq, PartialEq, Debug, uDebug)]
pub enum CertificateType {
    // "identity", issued by certificate authority
    // --> authentication
    PublicKey,
    // issued by attribute authority
    // --> authorization
    Attribute,
}

// pub enum CertificateCategory {
//     Authority,
//     Token,
//     Other,
// }

// #[derive(Clone, Default, Eq, PartialEq, Debug)]
// pub struct CertificateAttributes {
//     pub certificate_type CertificateType,
// }


#[derive(Clone, Default, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
pub struct DataAttributes {
    // application that manages the object
    // pub application: String<MAX_APPLICATION_NAME_LENGTH>,
    // DER-encoding of *type* of data object
    // pub object_id: ByteBuf<?>,
    pub kind: ShortData,
    pub value: LongData,
}

// TODO: In PKCS#11v3, this is a map (AttributeType: ulong -> (*void, len)).
// "An array of CK_ATTRIBUTEs is called a “template” and is used for creating, manipulating and searching for objects."
//
// Maybe we should put these attributes in an enum, and pass an `heapless::IndexSet` of attributes.
// How do we handle defaults?
//
// Lookup seems a bit painful, on the other hand a struct of options is wasteful.
#[derive(Copy, Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
pub struct KeyAttributes {
    // key_type: KeyType,
    // object_id: ByteBuf,
    // derive: bool, // can other keys be derived
    // local: bool, // generated on token, or copied from such
    // key_gen_mechanism: Mechanism, // only for local, how was key generated
    // allowed_mechanisms: Vec<Mechanism>,

    // never return naked private key
    sensitive: bool,
    // always_sensitive: bool,

    // do not even return wrapped private key
    extractable: bool,
    // never_extractable: bool,

    // do not save to disk
    persistent: bool,
}

impl Default for KeyAttributes {
    fn default() -> Self {
        Self {
            sensitive: true,
            // always_sensitive: true,
            extractable: false,
            // never_extractable: true,
            // cryptoki: token (vs session) object
            // cryptoki: default false
            persistent: false,
        }
    }
}

impl KeyAttributes {
    pub fn new() -> Self {
        Default::default()
    }
}

// TODO: How to store/check?
// TODO: Fix variant indices to keep storage stable!!
#[derive(Copy, Clone, Debug, uDebug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum KeyKind {
    // Aes256,
    Ed25519 = 1,
    Entropy32 = 2, // output of TRNG
    P256 = 3,
    SharedSecret32 = 4,  // or 256 (in bits)?
    SymmetricKey16 = 5,
    SymmetricKey32 = 6, // or directly: SharedSecret32 —DeriveKey(HmacSha256)-> SymmetricKey32 —Encrypt(Aes256)-> ...
    Symmetric32Nonce12 = 7,
    Symmetric24 = 8,
    Symmetric20 = 9,
    // ThirtytwoByteBuf,
}

impl core::convert::TryFrom<u8> for KeyKind {
    type Error = crate::error::Error;
    fn try_from(num: u8) -> Result<Self, Self::Error> {
        Ok(match num {
            1 => KeyKind::Ed25519,
            2 => KeyKind::Entropy32,
            3 => KeyKind::P256,
            4 => KeyKind::SharedSecret32,
            5 => KeyKind::SymmetricKey32,
            6 => KeyKind::Symmetric32Nonce12,
            _ => { return Err(crate::error::Error::CborError); }
        })
    }
}

#[derive(Copy, Clone, Debug, uDebug, Eq, PartialEq)]
pub enum KeyType {
    // Private,
    Public,
    Secret,
}

/// PhantomData to make it unconstructable
/// NB: Better to check in service that nothing snuck through!
#[derive(Clone, Default, Eq, PartialEq, Debug, uDebug, Deserialize, Serialize)]
pub struct Letters(pub ShortData, ());

impl TryFrom<ShortData> for Letters {
    type Error = crate::error::Error;

    fn try_from(bytes: ShortData) -> Result<Self, Self::Error> {
        if !&bytes.iter().all(|b| *b >= b'a' && *b <= b'z') {
            return Err(Self::Error::NotJustLetters);
        }
        Ok(Letters(bytes, ()))
    }
}

/// Opaque key handle
///
/// Ideally, this would be authenticated encryption
/// around the information that allows locating the key.
///
/// So e.g. users can't get at keys they don't own
///
#[derive(Copy, Clone, Eq, PartialEq, Debug, uDebug)]//, Deserialize, Serialize)]
pub struct ObjectHandle{
    pub object_id: UniqueId,
}

// #[derive(Clone, Eq, PartialEq, Debug, uDebug)]//, Deserialize, Serialize)]
// pub struct AutoDrop<STORE: crate::store::Store> {
//     handle: ObjectHandle,
//     store:  STORE,
// }

// impl<S: crate::store::Store> core::ops::Deref for AutoDrop<_> {
//     type Target = ObjectHandle;
//     fn deref(&self) -> &Self::Target {
//         &self.handle
//     }
// }

// impl<S: crate::store::Store> core::ops::DerefMut for AutoDrop {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.handle
//     }
// }

// impl<S: crate::store::Store> core::ops::Drop for AutoDrop {
//     fn drop(&mut self) {
//         // crate::store::delete_volatile(self.board.store(), &self.handle);
//     }
// }

// impl AutoDrop {
//     pub fn new(handle: ObjectHandle) -> Self {
//         Self(handle)
//     }
// }

impl serde::Serialize for ObjectHandle {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.object_id.0)
    }
}

impl<'de> serde::Deserialize<'de> for ObjectHandle {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ValueVisitor<'de>(PhantomData<&'de ()>);

        impl<'de> serde::de::Visitor<'de> for ValueVisitor<'de>
        {
            type Value = ObjectHandle;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("16 bytes")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> core::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                use core::convert::TryInto;
                if v.len() != 16 {
                    return Err(E::invalid_length(v.len(), &self))?;
                }
                Ok(ObjectHandle { object_id: UniqueId(v.try_into().unwrap()) } )
            }
        }

        deserializer.deserialize_bytes(ValueVisitor(PhantomData))
    }
}


#[derive(Clone, Eq, PartialEq, Debug, uDebug)]
pub enum ObjectType {
    Certificate(CertificateType),
    // TODO: maybe group under Feature(FeautureType), with FeatureType = Counter, ...
    // But what else??
    Counter,
    Data,
    Key(KeyType),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, uDebug)]
pub struct PublicKeyAttributes {
    // never return naked private key
    sensitive: bool,
    // always_sensitive: bool,

    // do not even return wrapped private key
    extractable: bool,
    // never_extractable: bool,

    // do not save to disk
    persistent: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, uDebug)]
pub struct PrivateKeyAttributes {
    // never return naked private key
    sensitive: bool,
    // always_sensitive: bool,

    // do not even return wrapped private key
    extractable: bool,
    // never_extractable: bool,

    // do not save to disk
    persistent: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
pub enum StorageLocation {
    Volatile,
    Internal,
    External,
}

#[derive(Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
pub struct StorageAttributes {
    // each object must have a unique ID
    // unique_id: UniqueId,

    // description of object
    // label: String<MAX_LABEL_LENGTH>,

    // // cryptoki: token (vs session) object
    // persistent: bool,
    pub persistence: StorageLocation,

    // cryptoki: user must be logged in
    // private: bool,

    // modifiable: bool,
    // copyable: bool,
    // destroyable: bool,

}

impl StorageAttributes {
    pub fn set_persistence(mut self, persistence: StorageLocation) -> Self {
        self.persistence = persistence;
        self
    }
}

impl StorageAttributes {
    // pub fn new(unique_id: UniqueId) -> Self {
    pub fn new() -> Self {
        Self {
            // unique_id,
            // label: String::new(),
            // persistent: false,

            persistence: StorageLocation::Volatile,

            // modifiable: true,
            // copyable: true,
            // destroyable: true,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
pub enum Mechanism {
    Aes256Cbc,
    Chacha8Poly1305,
    Ed25519,
    HmacSha256,
    // P256XSha256,
    P256,
    P256Prehashed,
    // clients can also do hashing by themselves
    Sha256,
    Tdes,
    Totp,
    Trng,
    X25519,
}

pub type LongData = ByteBuf<MAX_LONG_DATA_LENGTH>;
pub type MediumData = ByteBuf<MAX_MEDIUM_DATA_LENGTH>;
pub type ShortData = ByteBuf<MAX_SHORT_DATA_LENGTH>;

pub type Message = ByteBuf<MAX_MESSAGE_LENGTH>;

#[derive(Copy, Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
pub enum KeySerialization {
    // Asn1Der,
    Cose,
    EcdhEsHkdf256,
    Raw,
    Sec1,
}

pub type Signature = ByteBuf<MAX_SIGNATURE_LENGTH>;

#[derive(Copy, Clone, Eq, PartialEq, Debug, uDebug, Serialize, Deserialize)]
pub enum SignatureSerialization {
    Asn1Der,
    // Cose,
    Raw,
    // Sec1,
}

pub type SpecialId = u8;

// TODO: We rely on the RNG being good, so 15 zero bytes are improbable, and there
// are no clashes between trussed-generated and special (app-chosen) IDs.
// We may or may not want to model this in more detail as enum { Special, Random },
// and make sure the randomly generated IDs are never in the "special" range.
#[derive(Copy, Clone, Eq, PartialEq)]//, Deserialize, Serialize)]
pub struct UniqueId(pub(crate) [u8; 16]);

impl From<SpecialId> for UniqueId {
    fn from(special_id: u8) -> Self {
        let mut unique_id = [0u8; 16];
        // consider this a "little endian" u256, so "first" 256
        // keys are "special" or "well-known"
        unique_id[0] = special_id;
        Self(unique_id)
    }
}

impl UniqueId {
    pub fn hex(&self) -> [u8; 32] {
        let mut hex = [b'0'; 32];
        format_hex(&self.0, &mut hex);
        hex
    }

    pub fn try_from_hex(hex: &[u8]) -> core::result::Result<Self, ()> {
        // https://stackoverflow.com/a/52992629
        // (0..hex.len())
        // use hex::FromHex;
        // let maybe_bytes = <[u8; 16]>::from_hex(hex).map_err(|e| ());
        // maybe_bytes.map(|bytes| Self(ByteBuf::from_slice(&bytes).unwrap()))
        if (hex.len() & 1) == 1 {
            // panic!("hex len & 1 =  {}", hex.len() & 1);
            return Err(());
        }
        if hex.len() > 32 {
            // panic!("hex len {}", hex.len());
            return Err(());
        }
        // let hex = core::str::from_utf8(hex).map_err(|e| ())?;
        let hex = core::str::from_utf8(hex).unwrap();
        // let hex = core::str::from_utf8_unchecked(hex);
        let mut bytes = [0u8; 16];
        for i in 0..(hex.len() >> 1) {
            // bytes[i] = u8::from_str_radix(&hex[i..][..2], 16).map_err(|e| ())?;
            bytes[i] = u8::from_str_radix(&hex[2*i..][..2], 16).unwrap();
        }
        Ok(UniqueId(bytes))
    }
}

impl core::fmt::Debug for UniqueId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "UniqueId(")?;
        for ch in &self.hex() {
            write!(f, "{}", &(*ch as char))?;
        }
        write!(f, ")")
    }
}

impl ufmt::uDebug for UniqueId {
    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite + ?Sized,
    {
        <[u8] as ufmt::uDebug>::fmt(&self.hex(), f)
    }
}

pub type UserAttribute = ByteBuf<MAX_USER_ATTRIBUTE_LENGTH>;

// PANICS
// Also assumes buffer is initialised with b'0',
// not b'\0' if 2*data.len() < buffer.len()
const HEX_CHARS: &[u8] = b"0123456789abcdef";
fn format_hex(data: &[u8], mut buffer: &mut [u8]) {
    for byte in data.iter() {
        buffer[0] = HEX_CHARS[(byte >> 4) as usize];
        buffer[1] = HEX_CHARS[(byte & 0xf) as usize];
        buffer = &mut buffer[2..];
    }
}

