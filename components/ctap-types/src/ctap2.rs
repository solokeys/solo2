use serde::{Deserialize, Serialize};
use serde_indexed::{DeserializeIndexed, SerializeIndexed};

use crate::{Bytes, consts, String, Vec};
use crate::sizes::*;
pub use super::webauthn::*;

pub mod client_pin;
pub mod credential_management;

#[derive(Copy,Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CtapOptions {
    pub rk: bool,
    pub up: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uv: Option<bool>,
    pub plat: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_pin: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cred_protect: Option<bool>,
}

impl Default for CtapOptions {
    fn default() -> Self {
        Self {
            rk: false,
            up: true,
            uv: None,
            plat: false,
            client_pin: None,
            cred_protect: None,
        }
    }
}

// TODO: this is a bit weird to model...
// Need to be able to "skip unknown keys" in deserialization
#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
pub struct AuthenticatorExtensions {}

#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
pub struct AuthenticatorOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rk: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uv: Option<bool>,
}

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
// #[serde(rename_all = "camelCase")]
#[serde_indexed(offset = 1)]
pub struct GetAssertionParameters {
    pub rp_id: String<consts::U64>,
    pub client_data_hash: Bytes<consts::U32>,
    pub allow_list: Vec<PublicKeyCredentialDescriptor, consts::U8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<AuthenticatorExtensions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<AuthenticatorOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_auth: Option<Bytes<consts::U16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_protocol: Option<u32>,
}

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
// #[serde(rename_all = "camelCase")]
#[serde_indexed(offset = 1)]
pub struct MakeCredentialParameters {
    pub client_data_hash: Bytes<consts::U32>,
    pub rp: PublicKeyCredentialRpEntity,
    pub user: PublicKeyCredentialUserEntity,
    // e.g. webauthn.io sends 10
    pub pub_key_cred_params: Vec<PublicKeyCredentialParameters, consts::U12>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_list: Option<Vec<PublicKeyCredentialDescriptor, consts::U16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<AuthenticatorExtensions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<AuthenticatorOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_auth: Option<Bytes<consts::U16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_protocol: Option<u32>,
}

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
    pub credential_id: Bytes<consts::U128>,
    pub credential_public_key: crate::cose::PublicKey,//Bytes<COSE_KEY_LENGTH>,
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
        let l = crate::serde::cbor_serialize(&self.credential_public_key, &mut cbor_key).unwrap();
        bytes.extend_from_slice(&cbor_key[..l]).unwrap();

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

// NB: attn object definition / order at end of
// https://fidoalliance.org/specs/fido-v2.0-ps-20190130/fido-client-to-authenticator-protocol-v2.0-ps-20190130.html#authenticatorMakeCredential
// does not coincide with what python-fido2 expects in AttestationObject.__init__ *at all* :'-)
#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct AssertionResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<PublicKeyCredentialDescriptor>,
    pub auth_data: Bytes<AUTHENTICATOR_DATA_LENGTH>,
    pub signature: Bytes<ASN1_SIGNATURE_LENGTH>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<PublicKeyCredentialUserEntity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_credentials: Option<u32>,
}

#[derive(Clone,Debug,Eq,PartialEq,Serialize)]
pub struct NoneAttestationStatement {}

#[derive(Clone,Debug,Eq,PartialEq,Serialize)]
pub struct PackedAttestationStatement {
    pub alg: i32,
    pub sig: Bytes<ASN1_SIGNATURE_LENGTH>,
    pub x5c: Vec<Bytes<consts::U1024>, consts::U1>,
}

#[derive(Clone,Debug,Eq,PartialEq,Serialize)]
#[serde(untagged)]
pub enum AttestationStatement {
    None(NoneAttestationStatement),
    Packed(PackedAttestationStatement),
}

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct AttestationObject {
    pub fmt: String<consts::U32>,
    pub auth_data: Bytes<AUTHENTICATOR_DATA_LENGTH>,
    // pub att_stmt: Bytes<consts::U64>,
    pub att_stmt: AttestationStatement,
}

pub type AssertionResponses = Vec<AssertionResponse, consts::U8>;

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct AuthenticatorInfo {

    // 0x01
    pub versions: Vec<String<consts::U12>, consts::U3>,

    // 0x02
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String<consts::U11>, consts::U4>>,

    // 0x03
    // #[serde(with = "serde_bytes")]
    // #[serde(serialize_with = "serde_bytes::serialize", deserialize_with = "serde_bytes::deserialize")]
    // #[serde(serialize_with = "serde_bytes::serialize")]
    // pub(crate) aaguid: Vec<u8, consts::U16>,
    pub aaguid: Bytes<consts::U16>,

    // 0x04
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<CtapOptions>,

    // 0x05
    // TODO: this is actually the constant MESSAGE_SIZE
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_msg_size: Option<usize>,

    // 0x06
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_protocols: Option<Vec<u8, consts::U1>>,

    // 0x07
    // only in FIDO_2_1_PRE, see https://git.io/JeNxG
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_creds_in_list: Option<usize>,

    // 0x08
    // only in FIDO_2_1_PRE, see https://git.io/JeNxG
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cred_id_length: Option<usize>,

    // 0x09
    // only in FIDO_2_1_PRE, see https://git.io/JeNxG
    // can be: usb, nfc, ble, internal
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transports: Option<Vec<Bytes<consts::U8>, consts::U4>>,

    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) algorithms: Option<&'l[u8]>,
}

impl Default for AuthenticatorInfo {
    fn default() -> Self {
        let mut zero_aaguid = Vec::<u8, consts::U16>::new();
        zero_aaguid.resize_default(16).unwrap();
        let aaguid = Bytes::<consts::U16>::from(zero_aaguid);

        Self {
            versions: Vec::new(),
            extensions: None,
            aaguid: aaguid,
            // options: None,
            options: Some(CtapOptions::default()),
            max_msg_size: None, //Some(MESSAGE_SIZE),
            pin_protocols: None,
            max_creds_in_list: None,
            max_cred_id_length: None,
            transports: None,
            // algorithms: None,
        }
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
