use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::{ByteBuf, consts};
use crate::sizes::*;

pub mod client_pin;
pub mod credential_management;
pub mod get_assertion;
pub mod get_next_assertion;
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

#[derive(Clone,Debug,uDebug,Eq,PartialEq,Serialize,Deserialize)]
pub struct AuthenticatorOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rk: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Note: This flag asks to perform UV *within the authenticator*,
    /// for instance with biometrics or on-device PIN entry,
    /// use of pinAuth is implicit where required.
    pub uv: Option<bool>,
}

// #[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
// // #[serde(rename_all = "camelCase")]
// #[serde_indexed(offset = 1)]
// pub struct GetAssertionParameters {
//     pub rp_id: String<consts::U64>,
//     pub client_data_hash: ByteBuf<consts::U32>,
//     pub allow_list: Vec<PublicKeyCredentialDescriptor, consts::U8>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub extensions: Option<AuthenticatorExtensions>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub options: Option<AuthenticatorOptions>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub pin_auth: Option<ByteBuf<consts::U16>>,
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

pub type PinAuth = ByteBuf<consts::U16>;

// #[derive(Clone,Debug,Eq,PartialEq)]
// // #[serde(rename_all = "camelCase")]
// pub struct AuthenticatorData {
//     pub rp_id_hash: ByteBuf<consts::U32>,
//     pub flags: u8,
//     pub sign_count: u32,
//     // this can get pretty long
//     pub attested_credential_data: Option<ByteBuf<ATTESTED_CREDENTIAL_DATA_LENGTH>>,
//     // pub extensions: ?
// }

// impl AuthenticatorData {
//     pub fn serialize(&self) -> ByteBuf<AUTHENTICATOR_DATA_LENGTH> {
//         let mut bytes = Vec::<u8, AUTHENTICATOR_DATA_LENGTH>::new();

//         // 32 bytes, the RP id's hash
//         bytes.extend_from_slice(&self.rp_id_hash).unwrap();
//         // flags
//         bytes.push(self.flags).unwrap();
//         // signature counts as 32-bit unsigned big-endian integer.
//         bytes.extend_from_slice(&self.sign_count.to_be_bytes()).unwrap();
//         match &self.attested_credential_data {
//             Some(ref attested_credential_data) => {
//                 // finally the attested credential data
//                 bytes.extend_from_slice(&attested_credential_data).unwrap();
//             },
//             None => {},
//         }

//         ByteBuf::from(bytes)
//     }
// }

bitflags! {
    pub struct AuthenticatorDataFlags: u8 {
        const USER_PRESENCE = 1 << 0;
        const USER_VERIFIED = 1 << 2;
        const ATTESTED_CREDENTIAL_DATA = 1 << 6;
        const EXTENSION_DATA = 1 << 7;
    }
}

pub trait SerializeAttestedCredentialData {
    fn serialize(&self) -> ByteBuf<ATTESTED_CREDENTIAL_DATA_LENGTH>;
}

#[derive(Clone,Debug,Eq,PartialEq)]
// #[serde(rename_all = "camelCase")]
pub struct AuthenticatorData<A, E> {
    pub rp_id_hash: ByteBuf<consts::U32>,
    pub flags: AuthenticatorDataFlags,
    pub sign_count: u32,
    // this can get pretty long
    // pub attested_credential_data: Option<ByteBuf<ATTESTED_CREDENTIAL_DATA_LENGTH>>,
    pub attested_credential_data: Option<A>,
    pub extensions: Option<E>
}

pub type SerializedAuthenticatorData = ByteBuf<AUTHENTICATOR_DATA_LENGTH>;

// The reason for this non-use of CBOR is for compatibility with
// FIDO U2F authentication signatures.
impl<A: SerializeAttestedCredentialData, E: serde::Serialize> AuthenticatorData<A, E> {
    pub fn serialize(&self) -> SerializedAuthenticatorData {
        // let mut bytes = Vec::<u8, AUTHENTICATOR_DATA_LENGTH>::new();
        let mut bytes = SerializedAuthenticatorData::new();

        // 32 bytes, the RP id's hash
        bytes.extend_from_slice(&self.rp_id_hash).unwrap();
        // flags
        bytes.push(self.flags.bits()).unwrap();
        // signature counts as 32-bit unsigned big-endian integer.
        bytes.extend_from_slice(&self.sign_count.to_be_bytes()).unwrap();

        // the attested credential data
        if let Some(ref attested_credential_data) = &self.attested_credential_data {
            bytes.extend_from_slice(&attested_credential_data.serialize()).unwrap();
        }

        // the extensions data
        if let Some(extensions) = self.extensions.as_ref() {
            let mut extensions_buf = [0u8; 128];
            let ser = crate::serde::cbor_serialize(extensions, &mut extensions_buf).unwrap();
            bytes.extend_from_slice(ser).unwrap();
        }

        bytes
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
