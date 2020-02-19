use core::convert::TryFrom;

use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::types::*;

#[cfg(feature = "sha256")]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, P, V> for super::Sha256
{
    fn derive_key(resources: &mut ServiceResources<'a, 's, R, P, V>, request: request::DeriveKey)
        -> Result<reply::DeriveKey, Error>
    {
        let base_id = request.base_key.object_id;
        let mut shared_secret = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Secret, &base_id)?;
        resources.load_serialized_key(&path, &mut shared_secret)?;

        use sha2::digest::Digest;
        let mut hash = sha2::Sha256::new();
        hash.input(&shared_secret);
        let symmetric_key: [u8; 32] = hash.result().into();

        let key_id = resources.generate_unique_id()?;
        let path = resources.prepare_path_for_key(KeyType::Secret, &key_id)?;
        resources.store_serialized_key(&path, &symmetric_key)?;

        Ok(reply::DeriveKey {
            key: ObjectHandle { object_id: key_id },
        })
    }
}

// impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
// Agree<'a, 's, R, P, V> for super::P256 {}
#[cfg(not(feature = "sha256"))]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
DeriveKey<'a, 's, R, P, V> for super::Sha256 {}
// impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
// GenerateKey<'a, 's, R, P, V> for super::P256 {}
// impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
// Sign<'a, 's, R, P, V> for super::P256 {}
// impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
// Verify<'a, 's, R, P, V> for super::P256 {}
