use crate::{Bytes, consts, String, Vec};
use serde_indexed::{DeserializeIndexed, SerializeIndexed};

use super::{AuthenticatorExtensions, AuthenticatorOptions};
use crate::sizes::*;
use crate::webauthn::*;

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
// #[serde(rename_all = "camelCase")]
#[serde_indexed(offset = 1)]
pub struct Parameters {
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
