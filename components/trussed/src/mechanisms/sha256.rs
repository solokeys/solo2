use core::convert::TryInto;

use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::store::*;
use crate::types::*;

#[cfg(feature = "sha256")]
impl<R: RngRead, S: Store>
DeriveKey<R, S> for super::Sha256
{
    fn derive_key(resources: &mut ServiceResources<R, S>, request: request::DeriveKey)
        -> Result<reply::DeriveKey, Error>
    {
        let base_id = &request.base_key.object_id;

        let shared_secret: [u8; 32] = resources
            .load_key(KeyType::Secret, Some(KeyKind::SharedSecret32), base_id)?
            .value.as_slice()
            .try_into()
            .map_err(|_| Error::InternalError)?;

        // hash it
        use sha2::digest::Digest;
        let mut hash = sha2::Sha256::new();
        hash.input(&shared_secret);
        let symmetric_key: [u8; 32] = hash.result().into();

        let key_id = resources.store_key(
            request.attributes.persistence,
            KeyType::Secret, KeyKind::SymmetricKey32,
            &symmetric_key)?;
            // resources.generate_unique_id()?;

        Ok(reply::DeriveKey {
            key: ObjectHandle { object_id: key_id },
        })
    }
}

#[cfg(feature = "sha256")]
impl<R: RngRead, S: Store>
Hash<R, S> for super::Sha256
{
    fn hash(_resources: &mut ServiceResources<R, S>, request: request::Hash)
        -> Result<reply::Hash, Error>
    {
        use sha2::digest::Digest;
        let mut hash = sha2::Sha256::new();
        hash.input(&request.message);

        let mut hashed = ShortData::new();
        hashed.extend_from_slice(&hash.result()).unwrap();

        Ok(reply::Hash { hash: hashed } )
    }
}

// impl<R: RngRead, S: Store>
// Agree<R, S> for super::P256 {}
#[cfg(not(feature = "sha256"))]
impl<R: RngRead, S: Store>
DeriveKey<R, S> for super::Sha256 {}
// impl<R: RngRead, S: Store>
// GenerateKey<R, S> for super::P256 {}
// impl<R: RngRead, S: Store>
// Sign<R, S> for super::P256 {}
// impl<R: RngRead, S: Store>
// Verify<R, S> for super::P256 {}
