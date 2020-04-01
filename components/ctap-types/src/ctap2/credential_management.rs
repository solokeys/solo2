use crate::{Bytes, consts};
use serde_indexed::{DeserializeIndexed, SerializeIndexed};
use serde_repr::{Deserialize_repr, Serialize_repr};
use super::client_pin::PinV1Subcommand;

use crate::{
    cose::P256PublicKey,
    webauthn::{
        PublicKeyCredentialDescriptor,
        PublicKeyCredentialRpEntity,
        PublicKeyCredentialUserEntity,
    }
};

#[derive(Clone,Debug,uDebug,Eq,PartialEq,Serialize_repr,Deserialize_repr)]
#[repr(u8)]
pub enum Subcommand  {
    GetCredsMetadata = 0x01, // 1, 2
    EnumerateRpsBegin = 0x02, // 3, 4, 5
    EnumerateRpsGetNextRp = 0x03, //  3, 4
    EnumerateCredentialsBegin = 0x04, // 6, 7, 8 ,9, A
    EnumerateCredentialsGetNextCredential = 0x05, // 6, 7, 8, A
    DeleteCredential = 0x06, // -
}


#[derive(Clone,Debug,uDebug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct SubcommandParameters {
    // 0x01
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rp_id_hash: Option<Bytes<consts::U32>>,
    // 0x02
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<PublicKeyCredentialDescriptor>,
}

#[derive(Clone,Debug,uDebug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct Parameters {
    // 0x01
    pub sub_command: PinV1Subcommand,
    // 0x02
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_command_params: Option<SubcommandParameters>,
    // 0x03
    pub pin_protocol: u8,
    // 0x04
    pub pin_auth: Bytes<consts::U16>,
}

#[derive(Clone,Debug,uDebug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct Response {

    // Metadata

    // 0x01
    pub existing_resident_credentials_count: u32,
    // 0x02
    pub max_possible_remaining_residential_credentials_count: u32,

    // EnumerateRps

    // 0x03
    pub rp: PublicKeyCredentialRpEntity,
    // 0x04
    rp_id_hash: Bytes<consts::U32>,
    // 0x05
    pub total_rps: u32,

    // EnumerateCredentials given RP

    // 0x06
    pub user: PublicKeyCredentialUserEntity,
    // 0x07
    pub credential_id: PublicKeyCredentialDescriptor,
    // 0x08
    pub public_key: P256PublicKey,
    // 0x09
    pub total_credentials: u32,
    // 0x0A
    pub cred_protect: u8,
}
