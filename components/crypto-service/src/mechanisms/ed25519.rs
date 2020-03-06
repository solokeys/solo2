use core::convert::TryFrom;

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
Sign<'a, 's, R, I, E, V> for super::Ed25519
{
    fn sign(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::Sign)
        -> Result<reply::Sign, Error>
    {
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
