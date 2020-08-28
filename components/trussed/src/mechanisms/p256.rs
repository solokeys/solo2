use core::convert::{TryFrom, TryInto};

use crate::logger::{blocking};
use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::store::*;
use crate::types::*;

fn load_public_key<R: RngRead, S: Store>(resources: &mut ServiceResources<R, S>, key_id: &UniqueId)
    -> Result<nisty::PublicKey, Error> {

    let public_bytes = resources
        .load_key(KeyType::Public, Some(KeyKind::P256), &key_id)?
        .value;

    let public_bytes = match public_bytes.as_slice().len() {
        64 => {
            let mut public_bytes_ = [0u8; 64];
            public_bytes_.copy_from_slice(&public_bytes.as_ref());
            public_bytes_
        }
        _ => {
            return Err(Error::InternalError);
        }
    };

    let public_key = nisty::PublicKey::try_from(&public_bytes).map_err(|_| Error::InternalError)?;

    Ok(public_key)
}


#[cfg(feature = "p256")]
impl<R: RngRead, S: Store>
Agree<R, S> for super::P256
{
    fn agree(resources: &mut ServiceResources<R, S>, request: request::Agree)
        -> Result<reply::Agree, Error>
    {
        let private_id = request.private_key.object_id;
        let public_id = request.public_key.object_id;

        let keypair = load_keypair(resources, &private_id)?;
        let public_key = load_public_key(resources, &public_id)?;

        // THIS IS THE CORE
        blocking::info!("free/total RAMFS blocks: {:?}/{:?}",
            resources.store.vfs().available_blocks().unwrap(),
            resources.store.vfs().total_blocks(),
        ).ok();
        let shared_secret = keypair.secret.agree(&public_key).map_err(|_| Error::InternalError)?.to_bytes();

        let key_id = resources.store_key(
            request.attributes.persistence,
            KeyType::Secret, KeyKind::SharedSecret32,
            &shared_secret)?;

        // return handle
        Ok(reply::Agree { shared_secret: ObjectHandle { object_id: key_id } })
    }
}

#[cfg(feature = "p256")]
impl<R: RngRead, S: Store>
DeriveKey<R, S> for super::P256
{
    fn derive_key(resources: &mut ServiceResources<R, S>, request: request::DeriveKey)
        -> Result<reply::DeriveKey, Error>
    {
        let base_id = request.base_key.object_id;

        let keypair = load_keypair(resources, &base_id)?;

        let public_id = resources.store_key(
            request.attributes.persistence,
            KeyType::Public, KeyKind::P256,
            &keypair.public.to_bytes())?;

        Ok(reply::DeriveKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(feature = "p256")]
impl<R: RngRead, S: Store>
DeserializeKey<R, S> for super::P256
{
    fn deserialize_key(resources: &mut ServiceResources<R, S>, request: request::DeserializeKey)
        -> Result<reply::DeserializeKey, Error>
    {
          // - mechanism: Mechanism
          // - serialized_key: Message
          // - attributes: StorageAttributes

        let public_key = match request.format {
            KeySerialization::Cose => {
                // TODO: this should all be done upstream
                let cose_public_key: ctap_types::cose::P256PublicKey = crate::cbor_deserialize(
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

            KeySerialization::EcdhEsHkdf256 => {
                // TODO: this should all be done upstream
                let cose_public_key: ctap_types::cose::EcdhEsHkdf256PublicKey = crate::cbor_deserialize(
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

        let public_id = resources.store_key(
            request.attributes.persistence,
            KeyType::Public, KeyKind::P256,
            public_key.as_bytes())?;


        Ok(reply::DeserializeKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(feature = "p256")]
impl<R: RngRead, S: Store>
GenerateKey<R, S> for super::P256
{
    fn generate_key(resources: &mut ServiceResources<R, S>, request: request::GenerateKey)
        -> Result<reply::GenerateKey, Error>
    {
        // generate keypair
        let mut seed = [0u8; 32];
        resources.rng.read(&mut seed)
            .map_err(|_| Error::EntropyMalfunction)?;

        // let keypair = nisty::Keypair::generate_patiently(&seed);
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("p256 keypair with public key = {:?}", &keypair.public);

        // store keys
        let key_id = resources.store_key(
            request.attributes.persistence,
            KeyType::Secret, KeyKind::P256,
            &seed)?;

        // return handle
        Ok(reply::GenerateKey { key: ObjectHandle { object_id: key_id } })
    }
}

#[cfg(feature = "p256")]
impl<R: RngRead, S: Store>
SerializeKey<R, S> for super::P256
{
    fn serialize_key(resources: &mut ServiceResources<R, S>, request: request::SerializeKey)
        -> Result<reply::SerializeKey, Error>
    {

        let key_id = request.key.object_id;

        let public_key = load_public_key(resources, &key_id)?;

        let mut serialized_key = Message::new();
        match request.format {
            KeySerialization::EcdhEsHkdf256 => {
                let cose_pk = ctap_types::cose::EcdhEsHkdf256PublicKey {
                    x: ByteBuf::from_slice(&public_key.x_coordinate()).unwrap(),
                    y: ByteBuf::from_slice(&public_key.y_coordinate()).unwrap(),
                };
                crate::cbor_serialize_bytes(&cose_pk, &mut serialized_key).map_err(|_| Error::CborError)?;
            }
            KeySerialization::Cose => {
                let cose_pk = ctap_types::cose::P256PublicKey {
                    x: ByteBuf::from_slice(&public_key.x_coordinate()).unwrap(),
                    y: ByteBuf::from_slice(&public_key.y_coordinate()).unwrap(),
                };
                crate::cbor_serialize_bytes(&cose_pk, &mut serialized_key).map_err(|_| Error::CborError)?;
            }
            KeySerialization::Raw => {
                serialized_key.extend_from_slice(public_key.as_bytes()).map_err(|_| Error::InternalError)?;
            }
            KeySerialization::Sec1 => {
                serialized_key.extend_from_slice(
                    &public_key.compress()
                ).map_err(|_| Error::InternalError)?;
            }
            // _ => {
            //     return Err(Error::InternalError);
            // }
        };

        Ok(reply::SerializeKey { serialized_key })
    }
}

#[cfg(feature = "p256")]
impl<R: RngRead, S: Store>
Exists<R, S> for super::P256
{
    fn exists(resources: &mut ServiceResources<R, S>, request: request::Exists)
        -> Result<reply::Exists, Error>
    {
        let key_id = request.key.object_id;
        let exists = resources.exists_key(KeyType::Secret, Some(KeyKind::P256), &key_id);
        Ok(reply::Exists { exists })
    }
}

fn load_keypair<R: RngRead, S: Store>(resources: &mut ServiceResources<R, S>, key_id: &UniqueId)
    -> Result<nisty::Keypair, Error> {

    // blocking::info!("loading keypair").ok();
    let seed: [u8; 32] = resources
        .load_key(KeyType::Secret, Some(KeyKind::P256), &key_id)?
        .value.as_slice()
        .try_into()
        .map_err(|_| Error::InternalError)?;

    let keypair = nisty::Keypair::generate_patiently(&seed);
    // blocking::info!("seed: {:?}", &seed).ok();
    Ok(keypair)
}

#[cfg(feature = "p256")]
impl<R: RngRead, S: Store>
Sign<R, S> for super::P256
{
    fn sign(resources: &mut ServiceResources<R, S>, request: request::Sign)
        -> Result<reply::Sign, Error>
    {
        let key_id = request.key.object_id;

        // blocking::info!("loading keypair");
        let keypair = load_keypair(resources, &key_id)?;

        let native_signature = keypair.sign(&request.message);

        let our_signature = match request.format {
            SignatureSerialization::Asn1Der => {
                Signature::from_slice(&native_signature.to_asn1_der()).unwrap()
            }
            SignatureSerialization::Raw => {
                Signature::from_slice(&native_signature.to_bytes()).unwrap()
            }
        };
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("p256 sig = {:?}", &native_signature);
        // blocking::info!("p256 sig = {:?}", &our_signature).ok();

        blocking::info!("P256 signature:").ok();
        // blocking::info!("msg: {:?}", &request.message).ok();
        // blocking::info!("sig: {:?}", &our_signature).ok();

        // return signature
        Ok(reply::Sign { signature: our_signature })
    }
}

#[cfg(feature = "p256")]
impl<R: RngRead, S: Store>
Sign<R, S> for super::P256Prehashed
{
    fn sign(resources: &mut ServiceResources<R, S>, request: request::Sign)
        -> Result<reply::Sign, Error>
    {
        let key_id = request.key.object_id;

        let keypair = load_keypair(resources, &key_id).map_err(|e| {
            blocking::info!("load keypair error {:?}", e).ok();
            e
        })?;

        // blocking::info!("keypair loaded").ok();

        if request.message.len() != nisty::DIGEST_LENGTH {
            blocking::info!("wrong length").ok();
            return Err(Error::WrongMessageLength);
        }
        let message: [u8; 32] = request.message.as_slice().try_into().unwrap();
        blocking::info!("cast to 32B array").ok();

        let native_signature = keypair.sign_prehashed(&message);
        blocking::info!("signed").ok();

        let our_signature = match request.format {
            SignatureSerialization::Asn1Der => {
                Signature::from_slice(&native_signature.to_asn1_der()).unwrap()
            }
            SignatureSerialization::Raw => {
                Signature::from_slice(&native_signature.to_bytes()).unwrap()
            }
        };
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("p256 sig = {:?}", &native_signature);
        // blocking::info!("p256 sig = {:?}", &our_signature).ok();

        blocking::info!("P256 ph signature:").ok();
        // blocking::info!("msg: {:?}", &request.message).ok();
        // blocking::info!("sig: {:?}", &our_signature).ok();

        // return signature
        Ok(reply::Sign { signature: our_signature })
    }
}

#[cfg(feature = "p256")]
impl<R: RngRead, S: Store>
Verify<R, S> for super::P256
{
    fn verify(resources: &mut ServiceResources<R, S>, request: request::Verify)
        -> Result<reply::Verify, Error>
    {
        let key_id = request.key.object_id;

        let public_key = load_public_key(resources, &key_id)?;

        if request.signature.len() != nisty::SIGNATURE_LENGTH {
            return Err(Error::WrongSignatureLength);
        }

        let mut signature_array = [0u8; nisty::SIGNATURE_LENGTH];
        signature_array.copy_from_slice(&request.signature);

        if let SignatureSerialization::Raw = request.format {
        } else {
            // well more TODO
            return Err(Error::InvalidSerializationFormat);
        }

        let valid = public_key.verify(&request.message, &signature_array);
        Ok(reply::Verify { valid } )
    }
}

#[cfg(not(feature = "p256"))]
impl<R: RngRead, S: Store>
Agree<R, S> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<R: RngRead, S: Store>
DeriveKey<R, S> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<R: RngRead, S: Store>
GenerateKey<R, S> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<R: RngRead, S: Store>
Sign<R, S> for super::P256 {}
#[cfg(not(feature = "p256"))]
impl<R: RngRead, S: Store>
Verify<R, S> for super::P256 {}
