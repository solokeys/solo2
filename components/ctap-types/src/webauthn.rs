use serde::{Deserialize, Serialize};
use crate::{Bytes, consts, String};
use crate::sizes::*;

#[derive(Clone,Debug, Eq,PartialEq,Serialize,Deserialize)]
pub struct PublicKeyCredentialRpEntity {
    pub id: String<consts::U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String<consts::U64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String<consts::U64>>,
}

#[derive(Clone,Debug, Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicKeyCredentialUserEntity {
    pub id: Bytes<consts::U64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String<consts::U128>>,
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

#[derive(Clone,Debug, Eq,PartialEq,Serialize,Deserialize)]
pub struct PublicKeyCredentialParameters {
    pub alg: i32,
    #[serde(rename = "type")]
    pub key_type: String<consts::U32>,
}

#[derive(Clone,Debug, Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicKeyCredentialDescriptor {
    // NB: if this is too small, get a nasty error
    // See serde::error/custom for more info
    pub id: Bytes<MAX_CREDENTIAL_ID_LENGTH>,
    #[serde(rename = "type")]
    pub key_type: String<consts::U32>,
    // https://w3c.github.io/webauthn/#enumdef-authenticatortransport
    // transports: ...
}
