

use core::convert::TryFrom;

use crypto_service::{
    types::{
        ObjectHandle,
    },
};

pub(crate) use ctap_types::{
    Bytes, consts, Vec,
    // authenticator::{ctap1, ctap2, Error, Request, Response},
    authenticator::ctap2,
    ctap2::make_credential::CredentialProtectionPolicy,
    sizes::*,
};

use super::{Error, Result};

#[derive(Copy, Clone, Debug, serde::Deserialize, serde::Serialize)]
// #[derive(Copy, Clone, Debug, serde_indexed::DeserializeIndexed, serde_indexed::SerializeIndexed)]
pub enum CtapVersion {
    U2fV2,
    Fido20,
    Fido21Pre,
}

#[derive(Clone, Debug, Default)]
pub struct CredentialId(pub Bytes<MAX_CREDENTIAL_ID_LENGTH>);

// TODO: how to determine necessary size?
// pub type SerializedCredential = Bytes<consts::U512>;
// pub type SerializedCredential = Bytes<consts::U256>;
pub type SerializedCredential = crypto_service::types::Message;

#[derive(Clone, Debug)]
pub struct EncryptedSerializedCredential(pub crypto_service::api::reply::Encrypt);

impl TryFrom<EncryptedSerializedCredential> for CredentialId {
    type Error = Error;

    // fn try_from(esc: EncryptedSerializedCredential) -> Result<CredentialId> {
    //     let mut credential_id = crypto_service::types::Message::new();
    //     credential_id.extend_from_slice(&esc.0.tag).map_err(|_| Error::Other)?;
    //     credential_id.extend_from_slice(&esc.0.nonce).map_err(|_| Error::Other)?;
    //     credential_id.extend_from_slice(&esc.0.ciphertext).map_err(|_| Error::Other)?;
    //     Ok(CredentialId(credential_id))
    // }

    fn try_from(esc: EncryptedSerializedCredential) -> Result<CredentialId> {
        let mut credential_id = CredentialId::default();
        ctap_types::serde::cbor_serialize_bytes(&esc.0, &mut credential_id.0).map_err(|_| Error::Other)?;
        Ok(credential_id)
    }
}

impl TryFrom<CredentialId> for EncryptedSerializedCredential {
    // tag = 16B
    // nonce = 12B
    type Error = Error;

    fn try_from(cid: CredentialId) -> Result<EncryptedSerializedCredential> {
        let encrypted_serialized_credential = EncryptedSerializedCredential(
            ctap_types::serde::cbor_deserialize(&cid.0).map_err(|_| Error::InvalidCredential)?
        );
        Ok(encrypted_serialized_credential)
    }

    // fn try_from(cid: CredentialId) -> Result<EncryptedSerializedCredential> {
    //     if cid.0.len() < 28 {
    //         return Err(Error::InvalidCredential);
    //     }
    //     let tag = &cid.0[..16];
    //     let nonce = &cid.0[16..][..12];
    //     let cipher = &cid.0[28..];
    //     Ok(EncryptedSerializedCredential(crypto_service::api::reply::Encrypt {
    //         ciphertext: {
    //             let mut c = crypto_service::types::Message::new();
    //             c.extend_from_slice(cipher).map_err(|_| Error::Other)?;
    //             c
    //         },
    //         nonce: {
    //             let mut c = crypto_service::types::ShortData::new();
    //             c.extend_from_slice(nonce).map_err(|_| Error::Other)?;
    //             c
    //         },
    //         tag: {
    //             let mut c = crypto_service::types::ShortData::new();
    //             c.extend_from_slice(tag).map_err(|_| Error::Other)?;
    //             c
    //         },
    //     }))
    // }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Key {
    ResidentKey(ObjectHandle),
    WrappedKey(Bytes<consts::U92>),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum CredRandom {
    Resident(ObjectHandle),
    Wrapped(Bytes<consts::U92>),
}

// TODO: figure out sizes
// We may or may not follow https://github.com/satoshilabs/slips/blob/master/slip-0022.md
#[derive(Clone, Debug, serde_indexed::DeserializeIndexed, serde_indexed::SerializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct Credential {
    ctap: CtapVersion,

    // id, name, url
    rp: ctap_types::webauthn::PublicKeyCredentialRpEntity,
    // id, name, display_name
    user: ctap_types::webauthn::PublicKeyCredentialUserEntity,

    // can be just a counter, need to be able to determine "latest"
    creation_time: u32,
    // for stateless deterministic keys, it seems CTAP2 (but not CTAP1) makes signature counters optional
    use_counter: bool,
    // P256 or Ed25519
    pub algorithm: i32,
    // for RK in non-deterministic mode: refers to actual key
    // TODO(implement enums in cbor-deser): for all others, is a wrapped key
    // --> use above Key enum
    // #[serde(skip_serializing_if = "Option::is_none")]
    // key_id: Option<ObjectHandle>,
    pub key: Key,

    // extensions
    hmac_secret: Option<CredRandom>,
    pub cred_protect: CredentialProtectionPolicy,

    // TODO: add `sig_counter: Option<ObjectHandle>`,
    // and grant RKs a per-credential sig-counter.
}

pub type CredentialList = Vec<Credential, ctap_types::sizes::MAX_CREDENTIAL_COUNT_IN_LIST>;

impl Credential {
    pub fn new(
        ctap: CtapVersion,
        parameters: &ctap2::make_credential::Parameters,
        algorithm: i32,
        key: Key,
        timestamp: u32,
        hmac_secret: Option<CredRandom>,
        cred_protect: CredentialProtectionPolicy,
    )
        -> Self
    {
        Credential {
            ctap,
            rp: parameters.rp.clone(),
            user: parameters.user.clone(),

            creation_time: timestamp,
            use_counter: true,
            algorithm: algorithm,
            key,

            hmac_secret,
            cred_protect,
        }
    }

    pub fn serialize(&self) -> Result<SerializedCredential> {
        let mut serialized = SerializedCredential::new();
        let size = ctap_types::serde::cbor_serialize_bytes(self, &mut serialized).map_err(|_| Error::Other)?;
        Ok(serialized)
    }

    pub fn deserialize(bytes: &SerializedCredential) -> Result<Self> {
        // ctap_types::serde::cbor_deserialize(bytes).map_err(|_| Error::Other)
        Ok(ctap_types::serde::cbor_deserialize(bytes).unwrap())
    }
}
