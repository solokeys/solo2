#![allow(non_camel_case_types)]

use heapless::consts;

// TODO: this needs to be overridable.
// Should we use the "config crate that can have a replacement patched in" idea?

pub type MAX_APPLICATION_NAME_LENGTH = consts::U256;
pub type MAX_LONG_DATA_LENGTH = consts::U1024;
pub type MAX_MESSAGE_LENGTH = consts::U1024;
pub type MAX_OBJECT_HANDLES = consts::U16;
pub type MAX_LABEL_LENGTH = consts::U256;
pub type MAX_MEDIUM_DATA_LENGTH = consts::U256;
pub type MAX_PATH_LENGTH = consts::U256;
pub type MAX_SERIALIZED_KEY_LENGTH = consts::U128;
pub type MAX_SERVICE_CLIENTS = consts::U5;
pub type MAX_SHORT_DATA_LENGTH = consts::U128;
pub type MAX_SIGNATURE_LENGTH = consts::U72;
pub type MAX_USER_ATTRIBUTE_LENGTH = consts::U256;

pub const USER_ATTRIBUTE_NUMBER: u8 = 37;

