//! This is so nasty!
//!
//! We need to support 3DES to provide compatibility with Yubico's braindead
//! implementation of key management...

// use cortex_m_semihosting::{dbg, hprintln};

use core::convert::TryInto;
// needed to even get ::new() from des...
use block_cipher_trait::BlockCipher;

use crate::api::*;
use crate::error::Error;
use crate::service::*;
use crate::store::Store;
use crate::types::*;

#[cfg(feature = "tdes")]
impl<R: RngRead, S: Store> Encrypt<R, S> for super::Tdes
{
    /// Encrypts a single block. Let's hope we don't have to support ECB!!
    fn encrypt(resources: &mut ServiceResources<R, S>, request: request::Encrypt)
        -> Result<reply::Encrypt, Error>
    {
        if request.message.len() != 8 { return Err(Error::WrongMessageLength); }

        let key_id = request.key.object_id;

        let symmetric_key: [u8; 24] = resources
            .load_key(KeyType::Secret, None, &key_id)?
            .value.as_slice().try_into()
            .map_err(|_| Error::InternalError)?;

		let cipher = des::TdesEde3::new(GenericArray::from_slice(&symmetric_key));

		let mut message = request.message;
        cipher.encrypt_block(GenericArray::from_mut_slice(&mut message));

        Ok(reply::Encrypt { ciphertext: message, nonce: Default::default(), tag: Default::default() })
    }
}

#[cfg(feature = "tdes")]
impl<R: RngRead, S: Store> Decrypt<R, S> for super::Tdes
{
    /// Decrypts a single block. Let's hope we don't have to support ECB!!
    fn decrypt(resources: &mut ServiceResources<R, S>, request: request::Decrypt)
        -> Result<reply::Decrypt, Error>
    {
        if request.message.len() != 8 { return Err(Error::WrongMessageLength); }

        let key_id = request.key.object_id;

        let symmetric_key: [u8; 24] = resources
            .load_key(KeyType::Secret, None, &key_id)?
            .value.as_slice().try_into()
            .map_err(|_| Error::InternalError)?;

		let cipher = des::TdesEde3::new(GenericArray::from_slice(&symmetric_key));

        let mut message = request.message;
        cipher.decrypt_block(GenericArray::from_mut_slice(&mut message));

        Ok(reply::Decrypt { plaintext: Some(message) })
    }
}

#[cfg(feature = "tdes")]
impl<R: RngRead, S: Store>
UnsafeInjectKey<R, S> for super::Tdes
{
    fn unsafe_inject_key(resources: &mut ServiceResources<R, S>, request: request::UnsafeInjectKey)
        -> Result<reply::UnsafeInjectKey, Error>
    {
        if request.raw_key.len() != 24 {
            return Err(Error::WrongMessageLength);
        }

        // store it
        let key_id = resources.store_key(
            request.attributes.persistence,
            KeyType::Secret,
            KeyKind::Symmetric24,
            &request.raw_key,
        )?;

        Ok(reply::UnsafeInjectKey { key: ObjectHandle { object_id: key_id } })
    }
}

