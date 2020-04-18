use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::storage::*;
use crate::types::*;

#[cfg(feature = "sha256")]
impl<R: RngRead, S: Store>
DeriveKey<'_, R, S> for super::Sha256
{
    fn derive_key(resources: &mut ServiceResources<'_, R, S>, request: request::DeriveKey)
        -> Result<reply::DeriveKey, Error>
    {
        let base_id = request.base_key.object_id;
        let mut shared_secret = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Secret, &base_id)?;
        resources.load_key(&path, KeyKind::SharedSecret32, &mut shared_secret)?;

        use sha2::digest::Digest;
        let mut hash = sha2::Sha256::new();
        hash.input(&shared_secret);
        let symmetric_key: [u8; 32] = hash.result().into();

        let key_id = resources.generate_unique_id()?;
        let path = resources.prepare_path_for_key(KeyType::Secret, &key_id)?;
        resources.store_key(request.attributes.persistence, &path, KeyKind::SymmetricKey32, &symmetric_key)?;

        Ok(reply::DeriveKey {
            key: ObjectHandle { object_id: key_id },
        })
    }
}

#[cfg(feature = "sha256")]
impl<R: RngRead, S: Store>
Hash<'_, R, S> for super::Sha256
{
    fn hash(_resources: &mut ServiceResources<'_, R, S>, request: request::Hash)
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
// Agree<'_, R, S> for super::P256 {}
#[cfg(not(feature = "sha256"))]
impl<R: RngRead, S: Store>
DeriveKey<'_, R, S> for super::Sha256 {}
// impl<R: RngRead, S: Store>
// GenerateKey<'_, R, S> for super::P256 {}
// impl<R: RngRead, S: Store>
// Sign<'_, R, S> for super::P256 {}
// impl<R: RngRead, S: Store>
// Verify<'_, R, S> for super::P256 {}
