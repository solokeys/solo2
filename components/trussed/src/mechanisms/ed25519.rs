use core::convert::{TryFrom, TryInto};

use cortex_m_semihosting::hprintln;

use crate::api::*;
// use crate::config::*;
// use crate::debug;
use crate::error::Error;
use crate::service::*;
use crate::store::*;
use crate::types::*;

fn load_public_key<R: RngRead, S: Store>(resources: &mut ServiceResources<R, S>, key_id: &UniqueId)
    -> Result<salty::PublicKey, Error> {

    let public_bytes: [u8; 32] = resources
        .load_key(KeyType::Public, Some(KeyKind::Ed25519), &key_id)?
        .value.as_ref()
        .try_into()
        .map_err(|_| Error::InternalError)?;

    let public_key = salty::PublicKey::try_from(&public_bytes).map_err(|_| Error::InternalError)?;

    Ok(public_key)
}

fn load_keypair<R: RngRead, S: Store>(resources: &mut ServiceResources<R, S>, key_id: &UniqueId)
    -> Result<salty::Keypair, Error> {

    let seed: [u8; 32] = resources
        .load_key(KeyType::Secret, Some(KeyKind::Ed25519), &key_id)?
        .value.as_ref()
        .try_into()
        .map_err(|_| Error::InternalError)?;

    let keypair = salty::Keypair::from(&seed);
    // hprintln!("seed: {:?}", &seed).ok();
    Ok(keypair)
}

#[cfg(feature = "ed25519")]
impl<R: RngRead, S: Store>
DeriveKey<R, S> for super::Ed25519
{
    fn derive_key(resources: &mut ServiceResources<R, S>, request: request::DeriveKey)
        -> Result<reply::DeriveKey, Error>
    {
        let base_id = &request.base_key.object_id;
        let keypair = load_keypair(resources, base_id)?;

        let public_id = resources.store_key(
            request.attributes.persistence,
            KeyType::Public, KeyKind::Ed25519,
            keypair.public.as_bytes())?;

        Ok(reply::DeriveKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(feature = "ed25519")]
impl<R: RngRead, S: Store>
DeserializeKey<R, S> for super::Ed25519
{
    fn deserialize_key(resources: &mut ServiceResources<R, S>, request: request::DeserializeKey)
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

        let public_id = resources.store_key(
            request.attributes.persistence,
            KeyType::Public, KeyKind::Ed25519,
            public_key.as_bytes())?;

        Ok(reply::DeserializeKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(feature = "ed25519")]
impl<R: RngRead, S: Store>
GenerateKey<R, S> for super::Ed25519
{
    fn generate_key(resources: &mut ServiceResources<R, S>, request: request::GenerateKey)
        -> Result<reply::GenerateKey, Error>
    {
        let mut seed = [0u8; 32];
        resources.rng.read(&mut seed).map_err(|_| Error::EntropyMalfunction)?;

        // let keypair = salty::Keypair::from(&seed);
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("ed25519 keypair with public key = {:?}", &keypair.public);

        // store keys
        let key_id = resources.store_key(
            request.attributes.persistence,
            KeyType::Secret, KeyKind::Ed25519,
            &seed)?;

        // return handle
        Ok(reply::GenerateKey { key: ObjectHandle { object_id: key_id } })
    }
}

#[cfg(feature = "ed25519")]
impl<R: RngRead, S: Store>
SerializeKey<R, S> for super::Ed25519
{
    fn serialize_key(resources: &mut ServiceResources<R, S>, request: request::SerializeKey)
        -> Result<reply::SerializeKey, Error>
    {
        let key_id = request.key.object_id;
        let public_key = load_public_key(resources, &key_id)?;

        let mut serialized_key = Message::new();
        match request.format {
            KeySerialization::Cose => {
                let cose_pk = ctap_types::cose::Ed25519PublicKey {
                    // x: Bytes::try_from_slice(public_key.x_coordinate()).unwrap(),
                    // x: Bytes::try_from_slice(&buf).unwrap(),
                    x: Bytes::try_from_slice(public_key.as_bytes()).unwrap(),
                };
                crate::cbor_serialize_bytes(&cose_pk, &mut serialized_key).map_err(|_| Error::CborError)?;
            }

            KeySerialization::Raw => {
                serialized_key.extend_from_slice(public_key.as_bytes()).map_err(|_| Error::InternalError)?;
                // serialized_key.extend_from_slice(&buf).map_err(|_| Error::InternalError)?;
            }

            _ => { return Err(Error::InternalError); }
        }

        Ok(reply::SerializeKey { serialized_key })
    }
}

#[cfg(feature = "ed25519")]
impl<R: RngRead, S: Store>
Exists<R, S> for super::Ed25519
{
    fn exists(resources: &mut ServiceResources<R, S>, request: request::Exists)
        -> Result<reply::Exists, Error>
    {
        let key_id = request.key.object_id;

        let exists = resources.exists_key(KeyType::Secret, Some(KeyKind::Ed25519), &key_id);
        Ok(reply::Exists { exists })
    }
}

#[cfg(feature = "ed25519")]
impl<R: RngRead, S: Store>
Sign<R, S> for super::Ed25519
{
    fn sign(resources: &mut ServiceResources<R, S>, request: request::Sign)
        -> Result<reply::Sign, Error>
    {
        // Not so nice, expands to
        // `trussed::/home/nicolas/projects/solo-bee/components/trussed/src/mechanisms/ed25519.rs:151
        // Ed25519::Sign`, i.e. VEERY long
        // debug!("trussed::{}:{} Ed25519::Sign", file!(), line!()).ok();
        // debug!("trussed: Ed25519::Sign").ok();
        // if let SignatureSerialization::Raw = request.format {
        // } else {
        //     return Err(Error::InvalidSerializationFormat);
        // }

        let key_id = request.key.object_id;

        let keypair = load_keypair(resources, &key_id)?;

        let native_signature = keypair.sign(&request.message);
        let our_signature = Signature::try_from_slice(&native_signature.to_bytes()).unwrap();

        // hprintln!("Ed25519 signature:").ok();
        // hprintln!("msg: {:?}", &request.message).ok();
        // hprintln!("pk:  {:?}", &keypair.public.as_bytes()).ok();
        // hprintln!("sig: {:?}", &our_signature).ok();

        // return signature
        Ok(reply::Sign { signature: our_signature })
    }
}

#[cfg(feature = "ed25519")]
impl<R: RngRead, S: Store>
Verify<R, S> for super::Ed25519
{
    fn verify(resources: &mut ServiceResources<R, S>, request: request::Verify)
        -> Result<reply::Verify, Error>
    {
        if let SignatureSerialization::Raw = request.format {
        } else {
            return Err(Error::InvalidSerializationFormat);
        }

        if request.signature.len() != salty::constants::SIGNATURE_SERIALIZED_LENGTH {
            return Err(Error::WrongSignatureLength);
        }

        let key_id = request.key.object_id;
        let public_key = load_public_key(resources, &key_id)?;

        let mut signature_array = [0u8; salty::constants::SIGNATURE_SERIALIZED_LENGTH];
        signature_array.copy_from_slice(request.signature.as_ref());
        let salty_signature = salty::Signature::from(&signature_array);

        Ok(reply::Verify { valid:
            public_key.verify(&request.message, &salty_signature).is_ok()
        })
    }
}

#[cfg(not(feature = "ed25519"))]
impl<R: RngRead, S: Store>
DeriveKey<R, S> for super::Ed25519 {}
#[cfg(not(feature = "ed25519"))]
impl<R: RngRead, S: Store>
GenerateKey<R, S> for super::Ed25519 {}
#[cfg(not(feature = "ed25519"))]
impl<R: RngRead, S: Store>
Sign<R, S> for super::Ed25519 {}
#[cfg(not(feature = "ed25519"))]
impl<R: RngRead, S: Store>
Verify<R, S> for super::Ed25519 {}
