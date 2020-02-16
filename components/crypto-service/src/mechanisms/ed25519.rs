use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::types::*;

#[cfg(feature = "ed25519")]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, P, V> for super::Ed25519
{
    fn derive_key(resources: &mut ServiceResources<'a, 's, R, P, V>, request: request::DeriveKey)
        -> Result<reply::DeriveKey, Error>
    {
        let base_id = request.base_key.object_id;
        let mut seed = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Secret, &base_id)?;
        resources.load_serialized_key(&path, &mut seed)?;
        let keypair = salty::Keypair::from(&seed);
        let public_id = resources.generate_unique_id()?;
        let public_path = resources.prepare_path_for_key(KeyType::Public, &public_id)?;
        resources.store_serialized_key(&public_path, keypair.public.as_bytes())?;
        Ok(reply::DeriveKey {
            key: ObjectHandle { object_id: public_id },
        })
    }
}

#[cfg(not(feature = "ed25519"))]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, P, V> for super::Ed25519
{}
