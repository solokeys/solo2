use core::convert::TryInto;

#[cfg(feature = "semihosting")]
use cortex_m_semihosting::hprintln;

use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::storage::*;
use crate::types::*;

#[cfg(feature = "hmac-sha256")]
impl<R: RngRead, S: Store>
Sign<'_, R, S> for super::HmacSha256
{
    fn sign(resources: &mut ServiceResources<'_, R, S>, request: request::Sign)
        -> Result<reply::Sign, Error>
    {
        use sha2::Sha256;
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<Sha256>;

        let key_id = request.key.object_id;
        // let mut shared_secret = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Secret, &key_id)?;
        // let (serialized_key, _) = resources.load_key_unchecked(&path, KeyKind::SymmetricKey32, &mut shared_secret)?;
        let (serialized_key, _) = resources.load_key_unchecked(&path)?;
        let shared_secret = &serialized_key.value;
        let l = shared_secret.len();
        if (l & 0xf) != 0 {
            hprintln!("wrong key length, expected multiple of 16, got {}", l).ok();
            Err(Error::WrongKeyKind)?;
        }
        // resources.load_key(&path, KeyKind::SharedSecret32, &mut shared_secret)?;
        // resources.load_key(&path, KeyKind::SymmetricKey16, &mut shared_secret)?;

        // let mut mac = HmacSha256::new_varkey(&shared_secret)
        let mut mac = HmacSha256::new_varkey(shared_secret)
            .expect("HMAC can take key of any size");

        mac.input(&request.message);
        let result = mac.result();
        // To get underlying array use `code` method, but be carefull, since
        // incorrect use of the code value may permit timing attacks which defeat
        // the security provided by the `MacResult`
        let code_bytes: [u8; 32] = result.code().as_slice().try_into().unwrap();
        let signature = Signature::try_from_slice(&code_bytes).unwrap();

        // return signature
        Ok(reply::Sign { signature })

    }
}

#[cfg(feature = "hmac-sha256")]
impl<R: RngRead, S: Store>
GenerateKey<'_, R, S> for super::HmacSha256
{
    fn generate_key(resources: &mut ServiceResources<'_, R, S>, request: request::GenerateKey)
        -> Result<reply::GenerateKey, Error>
    {
        let mut seed = [0u8; 16];
        resources.rng.read(&mut seed).map_err(|_| Error::EntropyMalfunction)?;

        // let keypair = salty::Keypair::from(&seed);
        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("ed25519 keypair with public key = {:?}", &keypair.public);

        // generate unique ids
        let key_id = resources.generate_unique_id()?;

        // store keys
        let path = resources.prepare_path_for_key(KeyType::Secret, &key_id)?;
        resources.store_key(request.attributes.persistence, &path, KeyKind::SymmetricKey16, &seed)?;

        // return handle
        Ok(reply::GenerateKey { key: ObjectHandle { object_id: key_id } })
    }
}


#[cfg(not(feature = "hmac-sha256"))]
impl<R: RngRead, S: Store>
Sign<'_, R, S> for super::HmacSha256 {}
