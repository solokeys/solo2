#[cfg(feature = "semihosting")]
use cortex_m_semihosting::hprintln;

use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::types::*;

#[cfg(feature = "aes256-cbc")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Encrypt<'a, 's, R, I, E, V> for super::Aes256Cbc
{
    /// Encrypts the input *with zero IV*
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

        let zero_iv = [0u8; 16];
		let cipher = Aes256Cbc::new_var(&symmetric_key, &zero_iv).unwrap();

		// buffer must have enough space for message+padding
		let mut buffer = request.message.clone();
		// // copy message to the buffer
		// let pos = plaintext.len();
		// buffer[..pos].copy_from_slice(plaintext);
        let l = buffer.len();
        // hprintln!(" aes256cbc encrypting l = {}B: {:?}", l, &buffer).ok();

        // Encrypt message in-place.
        // &buffer[..pos] is used as a message and &buffer[pos..] as a reserved space for padding.
        // The padding space should be big enough for padding, otherwise method will return Err(BlockModeError).
		let ciphertext = cipher.encrypt(&mut buffer, l).unwrap();

        let ciphertext = Message::try_from_slice(&ciphertext).unwrap();
        Ok(reply::Encrypt { ciphertext, nonce: ShortData::new(), tag: ShortData::new()  })
    }
}

#[cfg(feature = "aes256-cbc")]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
WrapKey<'a, 's, R, I, E, V> for super::Aes256Cbc
{
    fn wrap_key(resources: &mut ServiceResources<'a, 's, R, I, E, V>, request: request::WrapKey)
        -> Result<reply::WrapKey, Error>
    {
        // TODO: need to check both secret and private keys
        let path = resources.prepare_path_for_key(KeyType::Secret, &request.key.object_id)?;
        let (serialized_key, _location) = resources.load_key_unchecked(&path)?;

        let mut message = Message::new();
        // crate::cbor_serialize_bytes(&serialized_key, &mut message).map_err(|_| Error::CborError)?;
        let message: Message = serialized_key.value.try_convert_into().map_err(|_| Error::InternalError)?;

        let encryption_request = request::Encrypt {
            mechanism: Mechanism::Aes256Cbc,
            key: request.wrapping_key.clone(),
            message,
            associated_data: ShortData::new(),
            nonce: None,
        };
        let encryption_reply = <super::Aes256Cbc>::encrypt(resources, encryption_request)?;

        let wrapped_key = encryption_reply.ciphertext;

        Ok(reply::WrapKey { wrapped_key })
    }
}

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

        let zero_iv = [0u8; 16];
		let cipher = Aes256Cbc::new_var(&symmetric_key, &zero_iv).unwrap();

		// buffer must have enough space for message+padding
		let mut buffer = request.message.clone();
		// // copy message to the buffer
		// let pos = plaintext.len();
		// buffer[..pos].copy_from_slice(plaintext);
        // let l = buffer.len();

        // Decrypt message in-place.
        // Returns an error if buffer length is not multiple of block size and
        // if after decoding message has malformed padding.
        // hprintln!("encrypted: {:?}", &buffer).ok();
        // hprintln!("symmetric key: {:?}", &symmetric_key).ok();
		let plaintext = cipher.decrypt(&mut buffer).unwrap();
        // hprintln!("decrypted: {:?}", &plaintext).ok();
        let plaintext = Message::try_from_slice(&plaintext).unwrap();

        Ok(reply::Decrypt { plaintext: Some(plaintext) })
    }
}

#[cfg(not(feature = "aes256-cbc"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Decrypt<'a, 's, R, I, E, V> for super::Aes256Cbc {}
#[cfg(not(feature = "aes256-cbc"))]
impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage>
Encrypt<'a, 's, R, I, E, V> for super::Aes256Cbc {}
