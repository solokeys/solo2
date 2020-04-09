use crate::{Bytes, consts};
use serde_indexed::{DeserializeIndexed, SerializeIndexed};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::cose::EcdhEsHkdf256PublicKey;

#[derive(Clone,Debug,uDebug,Eq,PartialEq,Serialize_repr,Deserialize_repr)]
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

#[derive(Clone,Debug,uDebug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct Parameters {
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
    pub key_agreement: Option<EcdhEsHkdf256PublicKey>,

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

#[derive(Clone,Debug,uDebug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct Response {
    // 0x01, like ClientPinParameters::key_agreement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_agreement: Option<EcdhEsHkdf256PublicKey>,

    // 0x02, encrypted `pinToken` using `sharedSecret`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_token: Option<Bytes<consts::U32>>,

    // 0x03, number of PIN attempts remaining before lockout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u8>,

}

#[cfg(test)]
mod tests {

    #[test]
    fn pin_v1_subcommand() {
        // NB: This does *not* work without serde_repr, as the
        // discriminant of a numerical enum does not have to coincide
        // with its assigned value.
        // E.g., for PinV1Subcommand, the first entry is set to
        // value 1, but its discriminant (which our normal serialization
        // to CBOR would output) is 0.
        // The following test would then fail, as [1] != [2]
        let mut buf = [0u8; 64];
        let example = super::PinV1Subcommand::GetKeyAgreement;
        let ser = crate::serde::cbor_serialize(&example, &mut buf).unwrap();
        assert_eq!(ser, &[0x02]);
    }
}
