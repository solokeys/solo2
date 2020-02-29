use crate::{Bytes, consts, String, Vec};
use serde::Serialize;
use serde_indexed::{DeserializeIndexed, SerializeIndexed};

use super::{AuthenticatorExtensions, AuthenticatorOptions};
use crate::sizes::*;
use crate::webauthn::*;

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
    pub extensions: Option<AuthenticatorExtensions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<AuthenticatorOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_auth: Option<Bytes<consts::U16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_protocol: Option<u32>,
}

// It would be logical to call this Reponse :)
pub type AttestationObject = Response;

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct Response {
    pub fmt: String<consts::U32>,
    pub auth_data: Bytes<AUTHENTICATOR_DATA_LENGTH>,
    // pub att_stmt: Bytes<consts::U64>,
    pub att_stmt: AttestationStatement,
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
