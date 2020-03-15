#![allow(non_camel_case_types)]
use crate::consts;

pub type ATTESTED_CREDENTIAL_DATA_LENGTH = consts::U512;
// // not sure why i can't use `::to_usize()` here?
// pub const ATTESTED_CREDENTIAL_DATA_LENGTH_BYTES: usize = 512;

pub type AUTHENTICATOR_DATA_LENGTH = consts::U512;
// pub const AUTHENTICATOR_DATA_LENGTH_BYTES: usize = 512;

pub type ASN1_SIGNATURE_LENGTH = consts::U77;
// pub const ASN1_SIGNATURE_LENGTH_BYTES: usize = 72;

pub type COSE_KEY_LENGTH = consts::U256;
// pub const COSE_KEY_LENGTH_BYTES: usize = 256;

pub type CREDENTIAL_ID_LENGTH = consts::U512;

pub const PACKET_SIZE: usize = 64;

// 7609 bytes
pub const MESSAGE_SIZE: usize = PACKET_SIZE - 7 + 128 * (PACKET_SIZE - 5);
