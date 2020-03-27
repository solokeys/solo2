use crate::{Bytes, consts, String, Vec};
use serde::{Deserialize, Serialize};
use serde_indexed::{DeserializeIndexed, SerializeIndexed};

use super::{AuthenticatorOptions, PinAuth};
use crate::cose::P256PublicKey;
use crate::sizes::*;
use crate::webauthn::*;

// #[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
// pub struct AuthenticatorExtensions {
//     #[serde(rename = "hmac-secret")]
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub hmac_secret: Option<bool>,
// }

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct HmacSecretInput {
    pub key_agreement: P256PublicKey,
    // *either* enc(salt1) *or* enc(salt1 || salt2)
    pub salt_enc: Bytes<consts::U64>,
    pub salt_auth: Bytes<consts::U16>,

}

#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
pub struct Extensions {
    #[serde(rename = "hmac-secret")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hmac_secret: Option<HmacSecretInput>,
}

pub type AllowList = Vec<PublicKeyCredentialDescriptor, MAX_CREDENTIAL_COUNT_IN_LIST>;

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
// #[serde(rename_all = "camelCase")]
#[serde_indexed(offset = 1)]
pub struct Parameters {
    pub rp_id: String<consts::U64>,
    pub client_data_hash: Bytes<consts::U32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_list: Option<AllowList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Extensions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<AuthenticatorOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_auth: Option<PinAuth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_protocol: Option<u32>,
}

// NB: attn object definition / order at end of
// https://fidoalliance.org/specs/fido-v2.0-ps-20190130/fido-client-to-authenticator-protocol-v2.0-ps-20190130.html#authenticatorMakeCredential
// does not coincide with what python-fido2 expects in AttestationObject.__init__ *at all* :'-)
#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<PublicKeyCredentialDescriptor>,
    pub auth_data: Bytes<AUTHENTICATOR_DATA_LENGTH>,
    pub signature: Bytes<ASN1_SIGNATURE_LENGTH>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<PublicKeyCredentialUserEntity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_credentials: Option<u32>,
}

pub type Responses = Vec<Response, consts::U8>;
