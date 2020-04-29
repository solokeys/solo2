use core::convert::{TryFrom, TryInto};
use cortex_m_semihosting::hprintln;

use crypto_service::{
    Client as CryptoClient,
    pipe::Syscall as CryptoSyscall,
    types::{
        ObjectHandle,
    },
};

pub(crate) use ctap_types::{
    Bytes, Bytes32, consts, String, Vec,
    // authenticator::{ctap1, ctap2, Error, Request, Response},
    authenticator::ctap2,
    ctap2::make_credential::CredentialProtectionPolicy,
    sizes::*,
    webauthn::PublicKeyCredentialDescriptor,
};

use crate::{
    Authenticator,
    Error,
    Result,
};

#[macro_use]
macro_rules! block {
    ($future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $future_result;
        loop {
            match future_result.poll() {
                core::task::Poll::Ready(result) => { break result; },
                core::task::Poll::Pending => {},
            }
        }
    }}
}


#[derive(Copy, Clone, Debug, serde::Deserialize, serde::Serialize)]
// #[derive(Copy, Clone, Debug, serde_indexed::DeserializeIndexed, serde_indexed::SerializeIndexed)]
pub enum CtapVersion {
    U2fV2,
    Fido20,
    Fido21Pre,
}

#[derive(Clone, Debug, ufmt::derive::uDebug, Default, serde::Serialize, serde::Deserialize)]
pub struct CredentialId(pub Bytes<MAX_CREDENTIAL_ID_LENGTH>);

// TODO: how to determine necessary size?
// pub type SerializedCredential = Bytes<consts::U512>;
// pub type SerializedCredential = Bytes<consts::U256>;
pub type SerializedCredential = crypto_service::types::Message;

#[derive(Clone, Debug)]
pub struct EncryptedSerializedCredential(pub crypto_service::api::reply::Encrypt);

impl TryFrom<EncryptedSerializedCredential> for CredentialId {
    type Error = Error;

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
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Key {
    ResidentKey(ObjectHandle),
    // THIS USED TO BE 92 NOW IT'S 96 or 97 or so... waddup?
    WrappedKey(Bytes<consts::U128>),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum CredRandom {
    Resident(ObjectHandle),
    Wrapped(Bytes<consts::U92>),
}

#[derive(Clone, Debug, serde_indexed::DeserializeIndexed, serde_indexed::SerializeIndexed)]
pub struct CredentialData {
    // id, name, url
    pub rp: ctap_types::webauthn::PublicKeyCredentialRpEntity,
    // id, name, display_name
    user: ctap_types::webauthn::PublicKeyCredentialUserEntity,

    // can be just a counter, need to be able to determine "latest"
    pub creation_time: u32,
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
    pub hmac_secret: Option<CredRandom>,
    pub cred_protect: CredentialProtectionPolicy,

    // TODO: add `sig_counter: Option<ObjectHandle>`,
    // and grant RKs a per-credential sig-counter.
}

// TODO: figure out sizes
// We may or may not follow https://github.com/satoshilabs/slips/blob/master/slip-0022.md
#[derive(Clone, Debug, serde_indexed::DeserializeIndexed, serde_indexed::SerializeIndexed)]
// #[serde_indexed(offset = 1)]
pub struct Credential {
    ctap: CtapVersion,
    pub data: CredentialData,
    nonce: Bytes<consts::U12>,
}

impl core::ops::Deref for Credential {
    type Target = CredentialData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

pub type CredentialList = Vec<Credential, ctap_types::sizes::MAX_CREDENTIAL_COUNT_IN_LIST>;

impl Into<PublicKeyCredentialDescriptor> for CredentialId {
    fn into(self) -> PublicKeyCredentialDescriptor {
        PublicKeyCredentialDescriptor {
            id: self.0,
            key_type: {
                let mut key_type = String::new();
                key_type.push_str("public-key").unwrap();
                key_type
            }
        }
    }
}

impl Credential {
    pub fn new(
        ctap: CtapVersion,
        parameters: &ctap2::make_credential::Parameters,
        algorithm: i32,
        key: Key,
        timestamp: u32,
        hmac_secret: Option<CredRandom>,
        cred_protect: CredentialProtectionPolicy,
        nonce: [u8; 12],
    )
        -> Self
    {
        hprintln!("credential for algorithm {}", algorithm).ok();
        let data = CredentialData {
            rp: parameters.rp.clone(),
            user: parameters.user.clone(),

            creation_time: timestamp,
            use_counter: true,
            algorithm: algorithm,
            key,

            hmac_secret,
            cred_protect,
        };

        Credential {
            ctap,
            data,
            nonce: Bytes::try_from_slice(&nonce).unwrap(),
        }
    }

    pub fn id<'a, S>(
        &self,
        crypto: &mut CryptoClient<'a, S>,
        key_encryption_key: &ObjectHandle,
    )
        -> Result<CredentialId>
    where
        S: CryptoSyscall,
    {
        let serialized_credential = self.serialize()?;
        let message = &serialized_credential;
        // info!("ser cred = {:?}", message).ok();

        let rp_id_hash: Bytes32 = syscall!(crypto.hash_sha256(&self.rp.id.as_ref()))
            .hash
            .try_convert_into().map_err(|_| Error::Other)?;

        let associated_data = &rp_id_hash[..];
        let nonce: [u8; 12] = self.nonce.as_ref().try_into().unwrap();
        let encrypted_serialized_credential = EncryptedSerializedCredential(
            syscall!(crypto.encrypt_chacha8poly1305(
                    key_encryption_key, message, associated_data, Some(&nonce))));
        let credential_id: CredentialId = encrypted_serialized_credential.try_into().unwrap();

        Ok(credential_id)
    }

    pub fn serialize(&self) -> Result<SerializedCredential> {
        let mut serialized = SerializedCredential::new();
        let _size = ctap_types::serde::cbor_serialize_bytes(self, &mut serialized).map_err(|_| Error::Other)?;
        Ok(serialized)
    }

    /// BIG TODO: currently, if the data is invalid (for instance, if we
    /// rotated our encryption key), then this will crash...
    pub fn deserialize(bytes: &SerializedCredential) -> Result<Self> {
        // ctap_types::serde::cbor_deserialize(bytes).map_err(|_| Error::Other)

        // Ok(ctap_types::serde::cbor_deserialize(bytes).unwrap())
        match ctap_types::serde::cbor_deserialize(bytes) {
            Ok(s) => Ok(s),
            Err(_) => {
                panic!("could not deserialize {:?}", bytes);
            }
        }
    }

    pub fn try_from<S, UP>(
        authnr: &mut Authenticator<'_, S, UP>,
        rp_id_hash: &Bytes<consts::U32>,
        descriptor: &PublicKeyCredentialDescriptor,
    )
        -> Result<Self>
    where
        S: CryptoSyscall,
        UP: crate::UserPresence
    {
        let encrypted_serialized = EncryptedSerializedCredential::try_from(
            CredentialId(descriptor.id.clone())
        )?;

        let kek = authnr.state.persistent.key_encryption_key(&mut authnr.crypto)?;

        let serialized = block!(authnr.crypto.decrypt_chacha8poly1305(
            // TODO: use RpId as associated data here?
            &kek,
            &encrypted_serialized.0.ciphertext,
            &rp_id_hash[..],
            &encrypted_serialized.0.nonce,
            &encrypted_serialized.0.tag,
        ).unwrap())
            .map_err(|_| Error::InvalidCredential)?.plaintext
            .ok_or(Error::InvalidCredential)?;

        let credential = Credential::deserialize(&serialized)
            .map_err(|_| Error::InvalidCredential)?;

        Ok(credential)
    }

    // Does not work, as it would use a new, different nonce!
    //
    // pub fn id(&self) -> Result<CredentialId> {
    //     let serialized_credential = self.serialize()?;
    //     let key = &self.key_encryption_key()?;
    //     let message = &serialized_credential;
    //     let associated_data = parameters.rp.id.as_bytes();
    //     let encrypted_serialized_credential = EncryptedSerializedCredential(
    //         syscall!(self.crypto.encrypt_chacha8poly1305(key, message, associated_data)));
    //     let credential_id: CredentialId = encrypted_serialized_credential.try_into().unwrap();
    //     credential_id
    // }

    // pub fn store(&self) -> Result<gt
    //     let serialized_credential = self.serialize()?;
    //     let mut prefix = crypto_service::types::ShortData::new();
    //     prefix.extend_from_slice(b"rk").map_err(|_| Error::Other)?;
    //     let prefix = Some(crypto_service::types::Letters::try_from(prefix).map_err(|_| Error::Other)?);
    //     let blob_id = syscall!(self.crypto.store_blob(
    //         prefix.clone(),
    //         // credential_id.0.clone(),
    //         serialized_credential.clone(),
    //         StorageLocation::Internal,
    //         Some(rp_id_hash.clone()),
    //     )).blob;

    //     blob_id
    // }
}
