use core::convert::{TryFrom, TryInto};

use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::types::*;

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Agree<'a, 's, R, I, E, V> for super::P256
{
    fn agree(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::Agree)
        -> Result<reply::Agree, Error>
    {
        let private_id = request.private_key.object_id;
        let public_id = request.public_key.object_id;

        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Private, &private_id)?;
        resources.load_key(&path, KeyKind::P256, &mut seed)?;
        let keypair = nisty::Keypair::generate_patiently(&seed);

        let mut public_bytes = [0u8; 64];
        let path = resources.prepare_path_for_key(KeyType::Public, &public_id)?;
        resources.load_key(&path, KeyKind::P256, &mut public_bytes)?;
        let public_key = nisty::PublicKey::try_from(&public_bytes).map_err(|_| Error::InternalError)?;

        // THIS IS THE CORE
        let shared_secret = keypair.secret.agree(&public_key).map_err(|_| Error::InternalError)?.to_bytes();

        let key_id = resources.generate_unique_id()?;
        let path = resources.prepare_path_for_key(KeyType::Secret, &key_id)?;
        resources.store_key(request.attributes.persistence, &path, KeyKind::SharedSecret32, &shared_secret)?;

        // return handle
        Ok(reply::Agree { shared_secret: ObjectHandle { object_id: key_id } })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, I, E, V> for super::P256
{
    fn derive_key(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::DeriveKey)
        -> Result<reply::DeriveKey, Error>
    {
        let base_id = request.base_key.object_id;
        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Private, &base_id)?;
        resources.load_key(&path, KeyKind::P256, &mut seed)?;
        let keypair = nisty::Keypair::generate_patiently(&seed);
        let public_id = resources.generate_unique_id()?;
        let public_path = resources.prepare_path_for_key(KeyType::Public, &public_id)?;
        resources.store_key(request.attributes.persistence, &public_path, KeyKind::P256, keypair.public.as_bytes())?;
        Ok(reply::DeriveKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
DeserializeKey<'a, 's, R, I, E, V> for super::P256
{
    fn deserialize_key(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::DeserializeKey)
        -> Result<reply::DeserializeKey, Error>
    {
          // - mechanism: Mechanism
          // - serialized_key: Message
          // - attributes: StorageAttributes

        let public_key = match request.format {
            KeySerialization::Cose => {
                // TODO: this should all be done upstream
                let cose_public_key: ctap_types::cose::P256PublicKey = crate::service::cbor_deserialize(
                    &request.serialized_key).map_err(|_| Error::CborError)?;
                let mut serialized_key = [0u8; 64];
                if cose_public_key.x.len() != 32 || cose_public_key.y.len() != 32 {
                    return Err(Error::InvalidSerializedKey);
                }

                serialized_key[..32].copy_from_slice(&cose_public_key.x);
                serialized_key[32..].copy_from_slice(&cose_public_key.y);

                let public_key = nisty::PublicKey::try_from(&serialized_key)
                    .map_err(|_| Error::InvalidSerializedKey)?;

                public_key
            }

            KeySerialization::Raw => {
                if request.serialized_key.len() != 64 {
                    return Err(Error::InvalidSerializedKey);
                }

                let mut serialized_key = [0u8; 64];
                serialized_key.copy_from_slice(&request.serialized_key[..64]);
                let public_key = nisty::PublicKey::try_from(&serialized_key)
                    .map_err(|_| Error::InvalidSerializedKey)?;

                public_key
            }

            _ => { return Err(Error::InternalError); }
        };

        let public_id = resources.generate_unique_id()?;
        let public_path = resources.prepare_path_for_key(KeyType::Public, &public_id)?;
        resources.store_key(request.attributes.persistence, &public_path, KeyKind::P256, public_key.as_bytes())?;

        Ok(reply::DeserializeKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
GenerateKey<'a, 's, R, I, E, V> for super::P256
{
    fn generate_key(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::GenerateKey)
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
        resources.store_key(request.attributes.persistence, &path, KeyKind::P256, &seed)?;

        // return handle
        Ok(reply::GenerateKey { key: ObjectHandle { object_id: key_id } })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
SerializeKey<'a, 's, R, I, E, V> for super::P256
{
    fn serialize_key(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::SerializeKey)
        -> Result<reply::SerializeKey, Error>
    {

        let key_id = request.key.object_id;
        let path = resources.prepare_path_for_key(KeyType::Public, &key_id)?;
        let mut buf = [0u8; 64];
        resources.load_key(&path, KeyKind::P256, &mut buf)?;

        let public_key = nisty::PublicKey::try_from(&buf).map_err(|_| Error::InternalError)?;

        let mut serialized_key = Message::new();
        match request.format {
            KeySerialization::Cose => {
                let cose_pk = ctap_types::cose::P256PublicKey {
                    x: Bytes::try_from_slice(&public_key.x_coordinate()).unwrap(),
                    y: Bytes::try_from_slice(&public_key.y_coordinate()).unwrap(),
                };
                serialized_key.resize_to_capacity();
                let size = crate::service::cbor_serialize(&cose_pk, &mut serialized_key).map_err(|_| Error::CborError)?;
                serialized_key.resize_default(size).map_err(|_| Error::InternalError)?;
            }
            KeySerialization::Raw => {
                serialized_key.extend_from_slice(public_key.as_bytes()).map_err(|_| Error::InternalError)?;
            }
            KeySerialization::Sec1 => {
                serialized_key.extend_from_slice(
                    &public_key.compress()
                ).map_err(|_| Error::InternalError)?;
            }
            _ => {
                return Err(Error::InternalError);
            }
        };

        Ok(reply::SerializeKey { serialized_key })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Sign<'a, 's, R, I, E, V> for super::P256
{
    fn sign(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::Sign)
        -> Result<reply::Sign, Error>
    {
        let key_id = request.key.object_id;

        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Private, &key_id)?;
        resources.load_key(&path, KeyKind::P256, &mut seed)?;

        let keypair = nisty::Keypair::generate_patiently(&seed);
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("p256 keypair with public key = {:?}", &keypair.public);

        let native_signature = keypair.sign(&request.message);
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("p256 sig = {:?}", &native_signature);
        let our_signature = Signature::try_from_slice(&native_signature.to_bytes()).unwrap();

        // return signature
        Ok(reply::Sign { signature: our_signature })
    }
}

#[cfg(feature = "p256")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Verify<'a, 's, R, I, E, V> for super::P256
{
    fn verify(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::Verify)
        -> Result<reply::Verify, Error>
    {
        let key_id = request.key.object_id;

        let mut serialized_key = [0u8; 64];
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("attempting path from {:?}", &key_id);
        let path = resources.prepare_path_for_key(KeyType::Public, &key_id)?;
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("attempting load from {:?}", &path);
        resources.load_key(&path, KeyKind::P256, &mut serialized_key)?;

        // println!("p256 serialized public key = {:?}", &serialized_key[..]);
        let public_key = nisty::PublicKey::try_from(&serialized_key).map_err(|_| Error::InternalError)?;
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("p256 public key = {:?}", &public_key);

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
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Agree<'a, 's, R, I, E, V> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, I, E, V> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
GenerateKey<'a, 's, R, I, E, V> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Sign<'a, 's, R, I, E, V> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Verify<'a, 's, R, I, E, V> for super::P256 {}
