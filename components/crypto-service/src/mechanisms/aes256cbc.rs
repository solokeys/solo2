use core::convert::TryFrom;
use core::convert::TryInto;

use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::types::*;

#[cfg(feature = "aes256-cbc")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Decrypt<'a, 's, R, I, E, V> for super::Aes256Cbc
{
    fn decrypt(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::Decrypt)
        -> Result<reply::Decrypt, Error>
    {
		use block_modes::{BlockMode, Cbc};
		// use block_modes::Cbc;
		use block_modes::block_padding::ZeroPadding;
		use aes::Aes256;

        // TODO: perhaps use NoPadding and have client pad, to emphasize spec-conformance?
        type Aes256Cbc = Cbc<Aes256, ZeroPadding>;

        let key_id = request.key.object_id;
        let mut symmetric_key = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Secret, &key_id)?;
        resources.load_key(&path, KeyKind::SymmetricKey32, &mut symmetric_key)?;

        let zero_iv = [0u8; 32];
		let cipher = Aes256Cbc::new_var(&symmetric_key, &zero_iv).unwrap();

		// buffer must have enough space for message+padding
		let mut buffer = request.message.clone();
		// // copy message to the buffer
		// let pos = plaintext.len();
		// buffer[..pos].copy_from_slice(plaintext);
        let l = buffer.len();

        // Decrypt message in-place.
        // Returns an error if buffer length is not multiple of block size and
        // if after decoding message has malformed padding.
		let plaintext = cipher.decrypt(&mut buffer).unwrap();
        let plaintext = Message::try_from_slice(&plaintext).unwrap();

        Ok(reply::Decrypt { plaintext: Ok(plaintext) })
    }
}

impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Encrypt<'a, 's, R, I, E, V> for super::Aes256Cbc
{
    fn encrypt(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::Encrypt)
        -> Result<reply::Encrypt, Error>
    {
		use block_modes::{BlockMode, Cbc};
		// use block_modes::Cbc;
		use block_modes::block_padding::ZeroPadding;
		use aes::Aes256;

        // TODO: perhaps use NoPadding and have client pad, to emphasize spec-conformance?
        type Aes256Cbc = Cbc<Aes256, ZeroPadding>;

        let key_id = request.key.object_id;
        let mut symmetric_key = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Secret, &key_id)?;
        resources.load_key(&path, KeyKind::SymmetricKey32, &mut symmetric_key)?;

        let zero_iv = [0u8; 32];
		let cipher = Aes256Cbc::new_var(&symmetric_key, &zero_iv).unwrap();

		// buffer must have enough space for message+padding
		let mut buffer = request.message.clone();
		// // copy message to the buffer
		// let pos = plaintext.len();
		// buffer[..pos].copy_from_slice(plaintext);
        let l = buffer.len();

        // Encrypt message in-place.
        // &buffer[..pos] is used as a message and &buffer[pos..] as a reserved space for padding.
        // The padding space should be big enough for padding, otherwise method will return Err(BlockModeError).
		let ciphertext = cipher.encrypt(&mut buffer, l).unwrap();

        let ciphertext = Message::try_from_slice(&ciphertext).unwrap();
        Ok(reply::Encrypt { ciphertext })
    }
}

#[cfg(not(feature = "aes256-cbc"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Decrypt<'a, 's, R, I, E, V> for super::Aes256Cbc {}
#[cfg(not(feature = "aes256-cbc"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Encrypt<'a, 's, R, I, E, V> for super::Aes256Cbc {}
