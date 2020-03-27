use core::convert::{TryFrom, TryInto};

use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::types::*;

#[cfg(feature = "ed25519")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, I, E, V> for super::Ed25519
{
    fn derive_key(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::DeriveKey)
        -> Result<reply::DeriveKey, Error>
    {
        let base_id = request.base_key.object_id;
        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Private, &base_id)?;
        resources.load_key(&path, KeyKind::Ed25519, &mut seed)?;
        let keypair = salty::Keypair::from(&seed);
        let public_id = resources.generate_unique_id()?;
        let public_path = resources.prepare_path_for_key(KeyType::Public, &public_id)?;
        resources.store_key(request.attributes.persistence, &public_path, KeyKind::Ed25519, keypair.public.as_bytes())?;
        Ok(reply::DeriveKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(feature = "ed25519")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
DeserializeKey<'a, 's, R, I, E, V> for super::Ed25519
{
    fn deserialize_key(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::DeserializeKey)
        -> Result<reply::DeserializeKey, Error>
    {
          // - mechanism: Mechanism
          // - serialized_key: Message
          // - attributes: StorageAttributes

        if request.format != KeySerialization::Raw {
            return Err(Error::InternalError);
        }

        if request.serialized_key.len() != 32 {
            return Err(Error::InvalidSerializedKey);
        }

        let serialized_key: [u8; 32] = request.serialized_key[..32].try_into().unwrap();
        let public_key = salty::PublicKey::try_from(&serialized_key)
            .map_err(|_| Error::InvalidSerializedKey)?;

        let public_id = resources.generate_unique_id()?;
        let public_path = resources.prepare_path_for_key(KeyType::Public, &public_id)?;
        resources.store_key(request.attributes.persistence, &public_path, KeyKind::Ed25519, public_key.as_bytes())?;

        Ok(reply::DeserializeKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(feature = "ed25519")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
GenerateKey<'a, 's, R, I, E, V> for super::Ed25519
{
    fn generate_key(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::GenerateKey)
        -> Result<reply::GenerateKey, Error>
    {
        let mut seed = [0u8; 32];
        resources.rng.read(&mut seed).map_err(|_| Error::EntropyMalfunction)?;

        // let keypair = salty::Keypair::from(&seed);
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("ed25519 keypair with public key = {:?}", &keypair.public);

        // generate unique ids
        let key_id = resources.generate_unique_id()?;

        // store keys
        let path = resources.prepare_path_for_key(KeyType::Private, &key_id)?;
        resources.store_key(request.attributes.persistence, &path, KeyKind::Ed25519, &seed)?;

        // return handle
        Ok(reply::GenerateKey { key: ObjectHandle { object_id: key_id } })
    }
}

#[cfg(feature = "ed25519")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
SerializeKey<'a, 's, R, I, E, V> for super::Ed25519
{
    fn serialize_key(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::SerializeKey)
        -> Result<reply::SerializeKey, Error>
    {
        let key_id = request.key.object_id;
        let path = resources.prepare_path_for_key(KeyType::Public, &key_id)?;
        let mut buf = [0u8; 32];
        resources.load_key(&path, KeyKind::Ed25519, &mut buf)?;

        // just a test that it's valid
        let public_key = salty::PublicKey::try_from(&buf).map_err(|_| Error::InternalError)?;

        let mut serialized_key = Message::new();
        match request.format {
            KeySerialization::Cose => {
                let cose_pk = ctap_types::cose::Ed25519PublicKey {
                    // x: Bytes::try_from_slice(public_key.x_coordinate()).unwrap(),
                    x: Bytes::try_from_slice(&buf).unwrap(),
                };
                crate::cbor_serialize_bytes(&cose_pk, &mut serialized_key).map_err(|_| Error::CborError)?;
            }

            KeySerialization::Raw => {
                serialized_key.extend_from_slice(public_key.as_bytes()).map_err(|_| Error::InternalError)?;
                serialized_key.extend_from_slice(&buf).map_err(|_| Error::InternalError)?;
            }

            _ => { return Err(Error::InternalError); }
        }

        Ok(reply::SerializeKey { serialized_key })
    }
}

#[cfg(feature = "ed25519")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Exists<'a, 's, R, I, E, V> for super::Ed25519
{
    fn exists(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::Exists)
        -> Result<reply::Exists, Error>
    {
        let key_id = request.key.object_id;

        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Private, &key_id)?;

        let exists = resources.load_key(&path, KeyKind::Ed25519, &mut seed).is_ok();
        Ok(reply::Exists { exists })
    }
}

#[cfg(feature = "ed25519")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Sign<'a, 's, R, I, E, V> for super::Ed25519
{
    fn sign(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::Sign)
        -> Result<reply::Sign, Error>
    {
        if let SignatureSerialization::Raw = request.format {
        } else {
            return Err(Error::InvalidSerializationFormat);
        }

        let key_id = request.key.object_id;

        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Private, &key_id)?;
        resources.load_key(&path, KeyKind::Ed25519, &mut seed)?;

        let keypair = salty::Keypair::from(&seed);
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("ed25519 keypair with public key = {:?}", &keypair.public);

        let native_signature = keypair.sign(&request.message);
        let our_signature = Signature::try_from_slice(&native_signature.to_bytes()).unwrap();

        // return signature
        Ok(reply::Sign { signature: our_signature })
    }
}

#[cfg(feature = "ed25519")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Verify<'a, 's, R, I, E, V> for super::Ed25519
{
    fn verify(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::Verify)
        -> Result<reply::Verify, Error>
    {
        if let SignatureSerialization::Raw = request.format {
        } else {
            return Err(Error::InvalidSerializationFormat);
        }

        let key_id = request.key.object_id;

        let mut serialized_key = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Public, &key_id)?;
        resources.load_key(&path, KeyKind::Ed25519, &mut serialized_key)?;

        let public_key = salty::PublicKey::try_from(&serialized_key).map_err(|_| Error::InternalError)?;
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("ed25519 public key = {:?}", &public_key);

        if request.signature.len() != salty::constants::SIGNATURE_SERIALIZED_LENGTH {
            return Err(Error::WrongSignatureLength);
        }

        let mut signature_array = [0u8; salty::constants::SIGNATURE_SERIALIZED_LENGTH];
        signature_array.copy_from_slice(request.signature.as_ref());
        let salty_signature = salty::Signature::from(&signature_array);

        Ok(reply::Verify { valid:
            public_key.verify(&request.message, &salty_signature).is_ok()
        })
    }
}

#[cfg(not(feature = "ed25519"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, I, E, V> for super::Ed25519 {}
#[cfg(not(feature = "ed25519"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
GenerateKey<'a, 's, R, I, E, V> for super::Ed25519 {}
#[cfg(not(feature = "ed25519"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Sign<'a, 's, R, I, E, V> for super::Ed25519 {}
#[cfg(not(feature = "ed25519"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Verify<'a, 's, R, I, E, V> for super::Ed25519 {}
