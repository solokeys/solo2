pub use heapless::{
    consts,
    String,
    Vec,
};

pub use heapless_bytes::Bytes;

pub use littlefs2::{
    fs::{Filesystem, FilesystemWith},
    driver::Storage as LfsStorage,
    io::Result as LfsResult,
};

use crate::config::*;

pub use crate::client::FutureResult;

// for counters use the pkcs#11 idea of
// a monotonic incrementing counter that
// "increments on each read" --> save +=1 operation

/// Opaque key handle
///
/// Ideally, this would be authenticated encryption
/// around the information that allows locating the key.
///
/// So e.g. users can't get at keys they don't own
///
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct KeyHandle{
    pub key_id: KeyId,
}

impl KeyHandle {
    pub fn unique_id(&self) -> KeyId {
        self.key_id
    }
}

type KeyId = [u8; 16];

// TODO: In PKCS#11v3, this is a map (AttributeType: ulong -> (*void, len)).
// "An array of CK_ATTRIBUTEs is called a “template” and is used for creating, manipulating and searching for objects."
//
// Maybe we should put these attributes in an enum, and pass an `heapless::IndexSet` of attributes.
// How do we handle defaults?
//
// Lookup seems a bit painful, on the other hand a struct of options is wasteful.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct KeyParameters {
    // never return naked private key
    sensitive: bool,
    // always_sensitive: bool,

    // do not even return wrapped private key
    extractable: bool,
    // never_extractable: bool,

    // do not save to disk
    persistent: bool,
}

impl Default for KeyParameters {
    fn default() -> Self {
        Self {
            sensitive: true,
            extractable: false,
            persistent: false,
        }
    }
}

impl KeyParameters {
    pub fn new() -> Self {
        Default::default()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Mechanism {
    Ed25519,
    // P256,
    // X25519,
}

pub type Message = Bytes<MAX_MESSAGE_LENGTH>;

pub type Signature = Bytes<MAX_SIGNATURE_LENGTH>;

