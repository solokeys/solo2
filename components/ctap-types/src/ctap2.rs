use serde::{Deserialize, Serialize};

use crate::{Bytes, consts, Vec};
use crate::sizes::*;

pub mod client_pin;
pub mod credential_management;
pub mod get_assertion;
pub mod get_info;
pub mod make_credential;

// TODO: this is a bit weird to model...
// Need to be able to "skip unknown keys" in deserialization
//
// I think we want to model this is a "set of enums",
// and allow skipping unknown enum entries during deserialization
//
// NB: This depends on the command
//
// We need two things:
// - skip unknown fields
// #[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
// pub struct AuthenticatorExtensions {
//     // #[serde(skip_serializing_if = "Option::is_none")]
//     // pub cred_protect:
// }

#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
pub struct AuthenticatorOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rk: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uv: Option<bool>,
}

// #[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
// // #[serde(rename_all = "camelCase")]
// #[serde_indexed(offset = 1)]
// pub struct GetAssertionParameters {
//     pub rp_id: String<consts::U64>,
//     pub client_data_hash: Bytes<consts::U32>,
//     pub allow_list: Vec<PublicKeyCredentialDescriptor, consts::U8>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub extensions: Option<AuthenticatorExtensions>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub options: Option<AuthenticatorOptions>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub pin_auth: Option<Bytes<consts::U16>>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub pin_protocol: Option<u32>,
// }

//// This is some pretty weird stuff ^^
//// Example serialization:
//// { 1: 2,  // kty (key type): tstr / int  [ 2 = EC2 = elliptic curve with x and y coordinate pair
////                                           1 = OKP = Octet Key Pair = for EdDSA
////          // kid, bstr
////   3: -7, // alg: tstr / int
//// [ 4:     // key_ops: tstr / int           1 = sign, 2 = verify, 3 = encrypt, 4 = decrypt, ...many more
////
////  // the curve: 1  = P-256
////  -1: 1,
////  // x-coordinate
////  -2: b'\xa0\xc3\x14\x06!\xefM\xcc\x06u\xf0\xf5v\x0bXa\xe6\xacm\x8d\xd9O`\xbd\x81\xf1\xe0_\x1a*\xdd\x9e',
////  // y-coordinate
////  -3: b'\xb4\xd4L\x94-\xbeVr\xe9C\x13u V\xf4t^\xe4.\xa2\x87I\xfe \xa4\xb0KY\x03\x00\x8c\x01'}
////
////  EdDSA
////   1: 1
////   3: -8,
////  -1: 6,
////  -2: public key bytes
//#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
//#[serde(rename_all = "camelCase")]
//pub struct CredentialPublicKey {
//}

// NOTE: This is not CBOR, it has a custom encoding...
// https://www.w3.org/TR/webauthn/#sec-attested-credential-data
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct AttestedCredentialData {
	pub aaguid: Bytes<consts::U16>,
    // this is where "unlimited non-resident keys" get stored
    // TODO: Model as actual credential ID, with ser/de to bytes (format is up to authenticator)
    pub credential_id: Bytes<CREDENTIAL_ID_LENGTH>,
    // pub credential_public_key: crate::cose::PublicKey,//Bytes<COSE_KEY_LENGTH>,
    pub credential_public_key: Bytes<COSE_KEY_LENGTH>,
}

impl AttestedCredentialData {
    pub fn serialize(&self) -> Bytes<ATTESTED_CREDENTIAL_DATA_LENGTH> {
        let mut bytes = Vec::<u8, ATTESTED_CREDENTIAL_DATA_LENGTH>::new();
        // 16 bytes, the aaguid
        bytes.extend_from_slice(&self.aaguid).unwrap();

        // byte length of credential ID as 16-bit unsigned big-endian integer.
        bytes.extend_from_slice(&(self.credential_id.len() as u16).to_be_bytes()).unwrap();
        // raw bytes of credential ID
        bytes.extend_from_slice(&self.credential_id[..self.credential_id.len()]).unwrap();

        // use existing `bytes` buffer
        let mut cbor_key = [0u8; 128];
        // CHANGE this back if credential_public_key is not serialized again
        // let l = crate::serde::cbor_serialize(&self.credential_public_key, &mut cbor_key).unwrap();
        // bytes.extend_from_slice(&cbor_key[..l]).unwrap();
        bytes.extend_from_slice(&self.credential_public_key).unwrap();

        Bytes::from(bytes)
    }
}

#[derive(Clone,Debug,Eq,PartialEq)]
// #[serde(rename_all = "camelCase")]
pub struct AuthenticatorData {
    pub rp_id_hash: Bytes<consts::U32>,
    pub flags: u8,
    pub sign_count: u32,
    // this can get pretty long
    pub attested_credential_data: Option<Bytes<ATTESTED_CREDENTIAL_DATA_LENGTH>>,
    // pub extensions: ?
}

impl AuthenticatorData {
    pub fn serialize(&self) -> Bytes<AUTHENTICATOR_DATA_LENGTH> {
        let mut bytes = Vec::<u8, AUTHENTICATOR_DATA_LENGTH>::new();

        // 32 bytes, the RP id's hash
        bytes.extend_from_slice(&self.rp_id_hash).unwrap();
        // flags
        bytes.push(self.flags).unwrap();
        // signature counts as 32-bit unsigned big-endian integer.
        bytes.extend_from_slice(&self.sign_count.to_be_bytes()).unwrap();
        match &self.attested_credential_data {
            Some(ref attested_credential_data) => {
                // finally the attested credential data
                bytes.extend_from_slice(&attested_credential_data).unwrap();
            },
            None => {},
        }

        Bytes::from(bytes)
    }
}

// // TODO: add Default and builder
// #[derive(Clone,Debug,Eq,PartialEq,Serialize)]
// pub struct AuthenticatorInfo<'l> {
//     pub(crate) versions: &'l[&'l str],
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub(crate) extensions: Option<&'l[&'l str]>,
//     // #[serde(serialize_with = "serde_bytes::serialize")]
//     pub(crate) aaguid: &'l [u8],//; 16],
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub(crate) options: Option<CtapOptions>,
//     // TODO: this is actually the constant MESSAGE_SIZE
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub(crate) max_msg_size: Option<usize>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub(crate) pin_protocols: Option<&'l[u8]>,

//     // not in the CTAP spec, but see https://git.io/JeNxG
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub(crate) max_creds_in_list: Option<usize>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub(crate) max_cred_id_length: Option<usize>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub(crate) transports: Option<&'l[u8]>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub(crate) algorithms: Option<&'l[u8]>,
// }

// pub enum Algorithm {
//     ES256,
//     EdDSA,
// }
