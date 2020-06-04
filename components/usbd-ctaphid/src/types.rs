pub use heapless::{consts, ArrayLength, String, Vec};
pub use heapless_bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_indexed::{DeserializeIndexed, SerializeIndexed};

use crate::{
    constants::{
        ATTESTED_CREDENTIAL_DATA_LENGTH,
        // ATTESTED_CREDENTIAL_DATA_LENGTH_BYTES,
        AUTHENTICATOR_DATA_LENGTH,
        // AUTHENTICATOR_DATA_LENGTH_BYTES,
        // COSE_KEY_LENGTH,
        MESSAGE_SIZE,
        ASN1_SIGNATURE_LENGTH,
    },
};

pub use cosey as cose;
pub mod ctap1;
pub mod ctap2;

/// buffer should be big enough to hold serialized object.
pub fn cbor_serialize<T: serde::Serialize>(
    object: &T,
    buffer: &mut [u8],
) -> core::result::Result<usize, serde_cbor::Error> {
    let writer = serde_cbor::ser::SliceWrite::new(buffer);
    let mut ser = serde_cbor::Serializer::new(writer);

    object.serialize(&mut ser)?;

    let writer = ser.into_inner();
    let size = writer.bytes_written();

    Ok(size)
}

pub fn cbor_deserialize<'de, T: serde::Deserialize<'de>>(
    buffer: &'de [u8],
) -> core::result::Result<T, ctapcbor::error::Error> {
    ctapcbor::de::from_bytes(buffer)
}


/// CTAP CBOR is crazy serious about canonical format.
/// If you change the order here, for instance python-fido2
/// will no longer parse the entire authenticatorGetInfo
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
                // Deserialized:
                // {1: b'-T\x18\xa8\xc1\xd3&\x90\xbf\x0f?\x11S/\x9f\xeeo\x8f\xde\xc8\xc7|\x82\xf3V\xdd\xc6\xe5\xce\x03\xe6k',
                //  2: {'id': 'example.org', 'name': 'example site'},
                //  3: {'id': b'they', 'name': 'example user'},
                //  4: [{'alg': -7, 'type': 'public-key'}],
                //  5: []}

#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
pub struct PublicKeyCredentialRpEntity {
    pub id: String<consts::U64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String<consts::U64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String<consts::U64>>,
}

#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicKeyCredentialUserEntity {
    pub id: Bytes<consts::U64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String<consts::U64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String<consts::U64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String<consts::U64>>,
}

impl PublicKeyCredentialUserEntity {
    pub fn from(id: Bytes<consts::U64>) -> Self {
        Self { id, icon: None, name: None, display_name: None }
    }
}

#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
pub struct PublicKeyCredentialParameters {
    pub alg: i32,
    #[serde(rename = "type")]
    pub key_type: String<consts::U10>,
}

#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicKeyCredentialDescriptor {
    pub id: Bytes<consts::U128>,
    #[serde(rename = "type")]
    pub key_type: String<consts::U10>,
    // https://w3c.github.io/webauthn/#enumdef-authenticatortransport
    // transports: ...
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
    pub credential_public_key: cose::PublicKey,//Bytes<COSE_KEY_LENGTH>,
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
        let l = cbor_serialize(&self.credential_public_key, &mut cbor_key).unwrap();
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
            max_msg_size: Some(MESSAGE_SIZE),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize() {
        let mut buffer = [0u8; 64];
        let writer = serde_cbor::ser::SliceWrite::new(&mut buffer);
        let mut ser = serde_cbor::Serializer::new(writer);

        let mut cdh = Vec::<u8, consts::U32>::new();
        cdh.extend_from_slice(b"1234567890ABCDEF").unwrap();
        Bytes::from(cdh).serialize(&mut ser).unwrap();

        // let writer = ser.into_inner();
        // let size = writer.bytes_written();
        // let buffer = writer.into_inner();

        // println!("serialized: {:#x?}", &buffer[..size]);
        // panic!("");
    }

    #[test]
    fn test_client_data_hash() {
        let mut minimal = [
            0x50u8, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38,
            0x39, 0x30, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, ];

        // This fails, but no good error message!
        // let mut client_data_hash: ByteVec<consts::U15> =

        let client_data_hash: Bytes<consts::U16> =
            serde_cbor::de::from_mut_slice(
                &mut minimal).unwrap();

        assert_eq!(client_data_hash, b"1234567890ABCDEF");
    }

    #[test]
    fn test_how_vec_dumps() {
        use core::str::FromStr;
        let cred_params = PublicKeyCredentialParameters {
            alg: -7,
            key_type: String::from_str("public-key").unwrap(),
        };
        let mut params: Vec<PublicKeyCredentialParameters, consts::U8> = Vec::new();
        params.push(cred_params).unwrap();

        let mut buffer = [0u8; 64];
        let writer = serde_cbor::ser::SliceWrite::new(&mut buffer);
        let mut ser = serde_cbor::Serializer::new(writer);
        params.serialize(&mut ser).unwrap();
        let writer = ser.into_inner();
        let size = writer.bytes_written();
        let buffer = writer.into_inner();
        assert_eq!(
            &[0x81u8,
                0xa2,
                    0x63, 0x61, 0x6c, 0x67, 0x26,
                    0x64, 0x74, 0x79, 0x70, 0x65, 0x6a, 0x70, 0x75, 0x62, 0x6c, 0x69, 0x63, 0x2d, 0x6b, 0x65, 0x79,
            ], &buffer[..size]);

        use serde::de;
        let mut deserializer = serde_cbor::de::Deserializer::from_mut_slice(&mut buffer[..size]);
        let _deser: Vec<PublicKeyCredentialParameters, consts::U8> = de::Deserialize::deserialize(&mut deserializer).unwrap();
    }

    #[test]
    fn test_make_credential_deser() {
        let mut buffer = [
        0xa4u8,

        0x1,
        0x50, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x30, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46,

        0x2,
        0xa1, 0x62, 0x69, 0x64, 0x73, 0x68, 0x74, 0x74, 0x70, 0x73, 0x3a, 0x2f,
        0x2f, 0x79, 0x61, 0x6d, 0x6e, 0x6f, 0x72, 0x64, 0x2e, 0x63, 0x6f, 0x6d,

        0x3,
        0xa1, 0x62, 0x69, 0x64, 0x47, 0x6e, 0x69, 0x63, 0x6b, 0x72, 0x61, 0x79,

        // fourth entry of struct (packed, offset 1 in ser/de)
        0x4,
            // array of...
            0x81,
                // struct (map)
                0xa2,
                    0x63, 0x61, 0x6c, 0x67, 0x26,
                    0x64, 0x74, 0x79, 0x70, 0x65, 0x6a, 0x70, 0x75, 0x62, 0x6c, 0x69, 0x63, 0x2d, 0x6b, 0x65, 0x79,
        ];

        use serde::de;
        let mut deserializer = serde_cbor::de::Deserializer::from_mut_slice(&mut buffer);
        let _make_cred_params: MakeCredentialParameters = de::Deserialize::deserialize(&mut deserializer).unwrap();

        // let make_cred_params: MakeCredentialParameters =
        //     serde_cbor::de::from_mut_slice(
        //         &mut buffer).unwrap();
        // assert!(make_cred_params.client_data_hash.len() > 0);
        // assert!(make_cred_params.second_client_data_hash.is_none());
        // assert!(make_cred_params.third_client_data_hash.len() > 0);
    }

    // #[test]
    // fn test_make_credential_params() {

    //     let mut buffer = [
    //         163, 2, 162, 98, 105, 100, 107, 101, 120, 97, 109, 112, 108, 101, 46,
    //         111, 114, 103, 100, 110, 97, 109, 101, 105, 69, 120, 97, 109, 112, 108,
    //         101, 82, 80, 3, 164, 98, 105, 100, 71, 3, 104, 32, 204, 154, 255, 165,
    //         100, 105, 99, 111, 110, 120, 31, 104, 116, 116, 112, 115, 58, 47, 47,
    //         119, 119, 119, 46, 119, 51, 46, 111, 114, 103, 47, 84, 82, 47, 119, 101,
    //         98, 97, 117, 116, 104, 110, 47, 100, 110, 97, 109, 101, 115, 67, 97, 108,
    //         108, 97, 32, 86, 105, 114, 103, 105, 110, 105, 101, 32, 68, 97, 110, 97,
    //         107, 100, 105, 115, 112, 108, 97, 121, 78, 97, 109, 101, 120, 29, 68, 105,
    //         115, 112, 108, 97, 121, 101, 100, 32, 67, 97, 108, 108, 97, 32, 86, 105,
    //         114, 103, 105, 110, 105, 101, 32, 68, 97, 110, 97, 4, 129, 162, 99, 97,
    //         108, 103, 38, 100, 116, 121, 112, 101, 106, 112, 117, 98, 108, 105, 99,
    //         45, 107, 101, 121];
    //     let mut buffer = [163, 2, 162, 98, 105, 100, 107, 101, 120, 97, 109, 112, 108, 101, 46, 111, 114, 103, 100, 110, 97, 109, 101, 105, 69, 120, 97, 109, 112, 108, 101, 82, 80, 3, 163, 98, 105, 100, 71, 3, 104, 32, 204, 154, 255, 165, 100, 110, 97, 109, 101, 115, 67, 97, 108, 108, 97, 32, 86, 105, 114, 103, 105, 110, 105, 101, 32, 68, 97, 110, 97, 107, 100, 105, 115, 112, 108, 97, 121, 78, 97, 109, 101, 120, 29, 68, 105, 115, 112, 108, 97, 121, 101, 100, 32, 67, 97, 108, 108, 97, 32, 86, 105, 114, 103, 105, 110, 105, 101, 32, 68, 97, 110, 97, 4, 129, 162, 99, 97, 108, 103, 38, 100, 116, 121, 112, 101, 106, 112, 117, 98, 108, 105, 99, 45, 107, 101, 121];

    //     use serde::de;
    //     let mut deserializer = serde_cbor::de::Deserializer::from_mut_slice(&mut buffer).packed_starts_with(1);
    //     let _make_cred_params: MakeCredentialParameters = de::Deserialize::deserialize(&mut deserializer).unwrap();
    // }
}
