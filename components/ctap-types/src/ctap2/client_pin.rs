use crate::{Bytes, consts};
use serde_indexed::{DeserializeIndexed, SerializeIndexed};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::cose::P256PublicKey;

#[derive(Clone,Debug,Eq,PartialEq,Serialize_repr,Deserialize_repr)]
#[repr(u8)]
pub enum PinV1Subcommand {
    GetRetries = 0x01,
    GetKeyAgreement = 0x02,
    SetPin = 0x03,
    ChangePin = 0x04,
    GetPinToken = 0x05,
}

// minimum PIN length: 4 unicode
// maximum PIN length: UTF-8 represented by <= 63 bytes
// maximum consecutive incorrect PIN attempts: 8

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct ClientPinParameters {
    // 0x01
    // PIN protocol version chosen by the client.
    // For this version of the spec, this SHALL be the number 1.
    pub pin_protocol: u8,

    // 0x02
    // The authenticator Client PIN sub command currently being requested
    pub sub_command: PinV1Subcommand,

    // 0x03
    // Public key of platformKeyAgreementKey.
    // Must contain "alg" parameter, must not contain any other optional parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_agreement: Option<P256PublicKey>,

    // 0x04
    // First 16 bytes of HMAC-SHA-256 of encrypted contents
    // using `sharedSecret`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_auth: Option<Bytes<consts::U16>>,

    // 0x05
    // Encrypted new PIN using `sharedSecret`.
    // (Encryption over UTF-8 representation of new PIN).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_pin_enc: Option<Bytes<consts::U64>>,

    // 0x06
    // Encrypted first 16 bytes of SHA-256 of PIN using `sharedSecret`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_hash_enc: Option<Bytes<consts::U64>>,

}

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct ClientPinResponse {
    // 0x01, like ClientPinParameters::key_agreement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_agreement: Option<P256PublicKey>,

    // 0x02, encrypted `pinToken` using `sharedSecret`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_token: Option<Bytes<consts::U64>>,

    // 0x03, number of PIN attempts remaining before lockout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u8>,

}
