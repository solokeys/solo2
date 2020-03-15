use crate::{Bytes, consts, String, Vec};

use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use serde_indexed::{DeserializeIndexed, SerializeIndexed};
use serde_repr::{Deserialize_repr, Serialize_repr};

use super::{AttestedCredentialData, AuthenticatorOptions};
use crate::sizes::*;
use crate::webauthn::*;

// // Approach 1:
// pub type AuthenticatorExtensions = heapless::LinearMap<String<consts::U11>, bool, consts::U2>;

 #[derive(Copy,Clone,Debug,Eq,PartialEq,Serialize_repr,Deserialize_repr)]
// #[derive(Clone,Debug,Eq,PartialEq,Serialize, Deserialize)]
// #[serde(tag = "credProtect")]
#[repr(u8)]
pub enum CredentialProtectionPolicy {
    // #[serde(rename = "userVerificationOptional")]
    Optional = 1,
    // #[serde(rename = "userVerificationOptionalWithCredentialIDList")] // <-- len = 44
    OptionalWithCredentialIdList = 2,
    // #[serde(rename = "userVerificationRequired")]
    Required = 3,
}

impl core::default::Default for CredentialProtectionPolicy {
    fn default() -> Self {
        CredentialProtectionPolicy::Optional
    }
}

// impl core::convert::TryFrom<&String<consts::U44>> for CredentialProtectionPolicy {
//     type Error = crate::authenticator::Error;

//     fn try_from(value: &String<consts::U44>) -> Result<Self, Self::Error> {
//         Ok(match value.as_str() {
//             "userVerificationOptional" => CredentialProtectionPolicy::Optional,
//             "userVerificationOptionalWithCredentialIDList" => CredentialProtectionPolicy::OptionalWithCredentialIdList,
//             "userVerificationRequired" => CredentialProtectionPolicy::Required,
//             _ => { return Err(Self::Error::InvalidParameter); }
//         })
//     }
// }

impl core::convert::TryFrom<u8> for CredentialProtectionPolicy {
    type Error = crate::authenticator::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => CredentialProtectionPolicy::Optional,
            2 => CredentialProtectionPolicy::OptionalWithCredentialIdList,
            3 => CredentialProtectionPolicy::Required,
            _ => { return Err(Self::Error::InvalidParameter); }
        })
    }
}

// Approach 2:
#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
pub struct Extensions {
    #[serde(rename = "hmac-secret")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hmac_secret: Option<bool>,

    #[serde(rename = "credProtect")]
    #[serde(skip_serializing_if = "Option::is_none")]
    // pub cred_protect: Option<CredentialProtectionPolicy>,
    pub cred_protect: Option<u8>,
    // #[serde(serialize_with = "u8::from")]
    // pub cred_protect: Option<u8>,
}

// // Approach 3:
// #[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
// pub enum AuthenticatorExtension {
//     #[serde(rename = "hmac-secret")]
//     HmacSecret(bool),
//     #[serde(rename = "credProtect")]
//     CredProtect(bool),
// }

// #[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
// pub struct AuthenticatorExtensions {
//     #[serde(flatten)]
//     pub extensions: Vec<AuthenticatorExtension, consts::U3>,
// }

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
// #[serde(rename_all = "camelCase")]
#[serde_indexed(offset = 1)]
pub struct Parameters {
    pub client_data_hash: Bytes<consts::U32>,
    pub rp: PublicKeyCredentialRpEntity,
    pub user: PublicKeyCredentialUserEntity,
    // e.g. webauthn.io sends 10
    pub pub_key_cred_params: Vec<PublicKeyCredentialParameters, consts::U12>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_list: Option<Vec<PublicKeyCredentialDescriptor, consts::U16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Extensions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<AuthenticatorOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_auth: Option<Bytes<consts::U16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_protocol: Option<u32>,
}

// It would be logical to call this Reponse :)
pub type AttestationObject = Response;

//
// TODO: We have `Option<T>`, use it to combine
// `fmt` and `att_stmt`!
//
// #[derive(Clone,Debug,Eq,PartialEq,Serialize)]
// #[serde(into = "ResponseExplicitEnumOption")]
// pub struct Response {
//     pub auth_data: Bytes<AUTHENTICATOR_DATA_LENGTH>,
//     pub att_stmt: Option<AttestationStatement>,
// }

bitflags! {
    pub struct Flags: u8 {
        const USER_PRESENCE = 1 << 0;
        const USER_VERIFIED = 1 << 2;
        const ATTESTED_CREDENTIAL_DATA = 1 << 6;
        const EXTENSION_DATA = 1 << 7;
    }
}

#[derive(Clone,Debug,Eq,PartialEq)]
// #[serde(rename_all = "camelCase")]
pub struct AuthenticatorData {
    pub rp_id_hash: Bytes<consts::U32>,
    pub flags: Flags,
    pub sign_count: u32,
    // this can get pretty long
    // pub attested_credential_data: Option<Bytes<ATTESTED_CREDENTIAL_DATA_LENGTH>>,
    pub attested_credential_data: Option<AttestedCredentialData>,
    pub extensions: Option<Extensions>
}

pub type SerializedAuthenticatorData = Bytes<AUTHENTICATOR_DATA_LENGTH>;

// The reason for this non-use of CBOR is for compatibility with
// FIDO U2F authentication signatures.
impl AuthenticatorData {
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
        if let Some(ref extensions) = &self.extensions {
            let mut extensions_buf = [0u8; 128];
            let l = crate::serde::cbor_serialize(&extensions, &mut extensions_buf).unwrap();
            bytes.extend_from_slice(&extensions_buf[..l]).unwrap();
        }

        bytes
    }
}

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct Response {
    pub fmt: String<consts::U32>,
    pub auth_data: SerializedAuthenticatorData,
    // pub att_stmt: Bytes<consts::U64>,
    pub att_stmt: AttestationStatement,
}

#[derive(Clone,Debug,Eq,PartialEq,Serialize)]
#[serde(untagged)]
pub enum AttestationStatement {
    None(NoneAttestationStatement),
    Packed(PackedAttestationStatement),
}

#[derive(Clone,Debug,Eq,PartialEq,Serialize)]
#[serde(untagged)]
pub enum AttestationStatementFormat {
    None,
    Packed,
    // Tpm,
    // AndroidKey,
    // AndroidSafetynet,
    // FidoU2f,
}

#[derive(Clone,Debug,Eq,PartialEq,Serialize)]
pub struct NoneAttestationStatement {}

#[derive(Clone,Debug,Eq,PartialEq,Serialize)]
pub struct PackedAttestationStatement {
    pub alg: i32,
    pub sig: Bytes<ASN1_SIGNATURE_LENGTH>,
    pub x5c: Vec<Bytes<consts::U1024>, consts::U1>,
}
