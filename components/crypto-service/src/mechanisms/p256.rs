use core::convert::TryFrom;

use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::types::*;

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
Agree<'a, 's, R, P, V> for super::P256
{
    fn agree(resources: &mut ServiceResources<'a, 's, R, P, V>, request: request::Agree)
        -> Result<reply::Agree, Error>
    {
        let private_id = request.private_key.object_id;
        let public_id = request.public_key.object_id;

        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Private, &private_id)?;
        resources.load_serialized_key(&path, &mut seed)?;
        let keypair = nisty::Keypair::generate_patiently(&seed);

        let mut public_bytes = [0u8; 64];
        let path = resources.prepare_path_for_key(KeyType::Public, &public_id)?;
        resources.load_serialized_key(&path, &mut public_bytes)?;
        let public_key = nisty::PublicKey::try_from(&public_bytes).map_err(|_| Error::InternalError)?;

        // THIS IS THE CORE
        let shared_secret = keypair.secret.agree(&public_key).map_err(|_| Error::InternalError)?.to_bytes();

        let key_id = resources.generate_unique_id()?;
        let path = resources.prepare_path_for_key(KeyType::Secret, &key_id)?;
        resources.store_serialized_key(&path, &shared_secret)?;

        // return handle
        Ok(reply::Agree { shared_secret: ObjectHandle { object_id: key_id } })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, P, V> for super::P256
{
    fn derive_key(resources: &mut ServiceResources<'a, 's, R, P, V>, request: request::DeriveKey)
        -> Result<reply::DeriveKey, Error>
    {
        let base_id = request.base_key.object_id;
        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Private, &base_id)?;
        resources.load_serialized_key(&path, &mut seed)?;
        let keypair = nisty::Keypair::generate_patiently(&seed);
        let public_id = resources.generate_unique_id()?;
        let public_path = resources.prepare_path_for_key(KeyType::Public, &public_id)?;
        resources.store_serialized_key(&public_path, keypair.public.as_bytes())?;
        Ok(reply::DeriveKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
GenerateKey<'a, 's, R, P, V> for super::P256
{
    fn generate_key(resources: &mut ServiceResources<'a, 's, R, P, V>, request: request::GenerateKey)
        -> Result<reply::GenerateKey, Error>
    {
        // generate keypair
        let mut seed = [0u8; 32];
        resources.rng.read(&mut seed)
            .map_err(|_| Error::EntropyMalfunction)?;

        // let keypair = nisty::Keypair::generate_patiently(&seed);
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("p256 keypair with public key = {:?}", &keypair.public);

        // generate unique ids
        let key_id = resources.generate_unique_id()?;

        // store keys
        let path = resources.prepare_path_for_key(KeyType::Private, &key_id)?;
        resources.store_serialized_key(&path, &seed)?;

        // return handle
        Ok(reply::GenerateKey { key: ObjectHandle { object_id: key_id } })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
Sign<'a, 's, R, P, V> for super::P256
{
    fn sign(resources: &mut ServiceResources<'a, 's, R, P, V>, request: request::Sign)
        -> Result<reply::Sign, Error>
    {
        let key_id = request.key.object_id;

        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Private, &key_id)?;
        resources.load_serialized_key(&path, &mut seed)?;

        let keypair = nisty::Keypair::generate_patiently(&seed);
        #[cfg(all(test, feature = "verbose-tests"))]
        println!("p256 keypair with public key = {:?}", &keypair.public);

        let native_signature = keypair.sign(&request.message);
        #[cfg(all(test, feature = "verbose-tests"))]
        println!("p256 sig = {:?}", &native_signature);
        let our_signature = Signature::try_from_slice(&native_signature.to_bytes()).unwrap();

        // return signature
        Ok(reply::Sign { signature: our_signature })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
Verify<'a, 's, R, P, V> for super::P256
{
    fn verify(resources: &mut ServiceResources<'a, 's, R, P, V>, request: request::Verify)
        -> Result<reply::Verify, Error>
    {
        let key_id = request.key.object_id;

        let mut serialized_key = [0u8; 64];
        #[cfg(all(test, feature = "verbose-tests"))]
        println!("attempting path from {:?}", &key_id);
        let path = resources.prepare_path_for_key(KeyType::Public, &key_id)?;
        #[cfg(all(test, feature = "verbose-tests"))]
        println!("attempting load from {:?}", &path);
        resources.load_serialized_key(&path, &mut serialized_key)?;

        // println!("p256 serialized public key = {:?}", &serialized_key[..]);
        let public_key = nisty::PublicKey::try_from(&serialized_key).map_err(|_| Error::InternalError)?;
        #[cfg(all(test, feature = "verbose-tests"))]
        println!("p256 public key = {:?}", &public_key);

        if request.signature.len() != nisty::SIGNATURE_LENGTH {
            return Err(Error::WrongSignatureLength);
        }

        let mut signature_array = [0u8; nisty::SIGNATURE_LENGTH];
        signature_array.copy_from_slice(&request.signature);

        let valid = public_key.verify(&request.message, &signature_array);
        Ok(reply::Verify { valid } )
    }
}

#[cfg(not(feature = "p256"))]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
Agree<'a, 's, R, P, V> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, P, V> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
GenerateKey<'a, 's, R, P, V> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
Sign<'a, 's, R, P, V> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
Verify<'a, 's, R, P, V> for super::P256 {}
