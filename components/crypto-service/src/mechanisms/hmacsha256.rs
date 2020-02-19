use core::convert::TryFrom;
use core::convert::TryInto;

use crate::api::*;
// use crate::config::*;
use crate::error::Error;
use crate::service::*;
use crate::types::*;

#[cfg(feature = "hmac-sha256")]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
Sign<'a, 's, R, P, V> for super::HmacSha256
{
    fn sign(resources: &mut ServiceResources<'a, 's, R, P, V>, request: request::Sign)
        -> Result<reply::Sign, Error>
    {
        use sha2::Sha256;
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<Sha256>;

        let key_id = request.key.object_id;
        let mut shared_secret = [0u8; 32];
        let path = resources.prepare_path_for_key(KeyType::Secret, &key_id)?;
        // resources.load_serialized_key(&path, KeyKind::SharedSecret32, &mut shared_secret)?;
        resources.load_serialized_key(&path, KeyKind::SymmetricKey32, &mut shared_secret)?;

        let mut mac = HmacSha256::new_varkey(&shared_secret)
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

#[cfg(not(feature = "hmac-sha256"))]
impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage>
Sign<'a, 's, R, P, V> for super::HmacSha256 {}
