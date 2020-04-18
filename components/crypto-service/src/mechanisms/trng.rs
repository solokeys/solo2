use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::storage::*;
use crate::types::*;

#[cfg(feature = "trng")]
impl<R: RngRead, S: Store>
GenerateKey<R, S> for super::Trng
{
    fn generate_key(resources: &mut ServiceResources<R, S>, request: request::GenerateKey)
        -> Result<reply::GenerateKey, Error>
    {
        // generate entropy
        let mut entropy = [0u8; 32];
        resources.rng.read(&mut entropy)
            .map_err(|_| Error::EntropyMalfunction)?;

        // generate unique ids
        let key_id = resources.generate_unique_id()?;

        // store keys
        let path = resources.prepare_path_for_key(KeyType::Private, &key_id)?;
        resources.store_key(request.attributes.persistence, &path, KeyKind::Entropy32, &entropy)?;

        Ok(reply::GenerateKey { key: ObjectHandle { object_id: key_id } })
    }
}

