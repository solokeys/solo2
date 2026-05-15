//! PIN / key-agreement helpers for FIDO2 tests.

use aes::Aes256;
use cbc::cipher::{block_padding::NoPadding, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use cosey::EcdhEsHkdf256PublicKey;
use ctap_types::ctap2::{self, Request, Response};
use hmac::{Hmac, Mac};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{ecdh::diffie_hellman, PublicKey, SecretKey};
use rand_core::OsRng;
use serde_cbor::value::to_value;
use serde_cbor::Value;
use sha2::{Digest, Sha256};

use super::transport::TestAuthenticator;

type HmacSha256 = Hmac<Sha256>;
type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

fn client_pin_request_from_value(value: Value) -> ctap2::client_pin::Request<'static> {
    let encoded = serde_cbor::to_vec(&value).expect("serialize ClientPin request");
    let leaked: &'static [u8] = Vec::leak(encoded);
    serde_cbor::from_slice(leaked).expect("deserialize ClientPin request")
}

fn client_pin_request(
    sub_command: ctap2::client_pin::PinV1Subcommand,
    key_agreement: Option<&EcdhEsHkdf256PublicKey>,
    pin_auth: Option<Vec<u8>>,
    new_pin_enc: Option<Vec<u8>>,
    pin_hash_enc: Option<Vec<u8>>,
    permissions: Option<u8>,
) -> ctap2::client_pin::Request<'static> {
    let mut entries = vec![
        (Value::Integer(1), Value::Integer(1)),
        (Value::Integer(2), Value::Integer(sub_command as i128)),
    ];
    if let Some(key_agreement) = key_agreement {
        entries.push((
            Value::Integer(3),
            to_value(key_agreement).expect("serialize key agreement"),
        ));
    }
    if let Some(pin_auth) = pin_auth {
        entries.push((Value::Integer(4), Value::Bytes(pin_auth)));
    }
    if let Some(new_pin_enc) = new_pin_enc {
        entries.push((Value::Integer(5), Value::Bytes(new_pin_enc)));
    }
    if let Some(pin_hash_enc) = pin_hash_enc {
        entries.push((Value::Integer(6), Value::Bytes(pin_hash_enc)));
    }
    if let Some(permissions) = permissions {
        entries.push((Value::Integer(9), Value::Integer(permissions as i128)));
    }
    client_pin_request_from_value(Value::Map(entries.into_iter().collect()))
}

pub struct PinSession {
    token: Vec<u8>,
}

impl PinSession {
    pub fn set_pin(authn: &mut dyn TestAuthenticator, pin: &str) {
        Self::try_set_pin(authn, pin).expect("SetPin should succeed");
    }

    pub fn try_set_pin(authn: &mut dyn TestAuthenticator, pin: &str) -> Result<(), ctap2::Error> {
        let key_agreement = get_authenticator_key_agreement(authn);
        let shared_secret = establish_shared_secret(&key_agreement);
        let new_pin_enc = encrypt_pin(shared_secret.as_slice(), pin);
        let pin_auth = hmac_left_16(shared_secret.as_slice(), &new_pin_enc);

        let platform_key = key_agreement_from_public(&shared_secret.platform_public);
        let request = client_pin_request(
            ctap2::client_pin::PinV1Subcommand::SetPin,
            Some(&platform_key),
            Some(pin_auth.to_vec()),
            Some(new_pin_enc),
            None,
            None,
        );

        authn.call_ctap2(&Request::ClientPin(request)).map(|_| ())
    }

    pub fn change_pin(authn: &mut dyn TestAuthenticator, old_pin: &str, new_pin: &str) {
        Self::try_change_pin(authn, old_pin, new_pin).expect("ChangePin should succeed");
    }

    pub fn try_change_pin(
        authn: &mut dyn TestAuthenticator,
        old_pin: &str,
        new_pin: &str,
    ) -> Result<(), ctap2::Error> {
        let key_agreement = get_authenticator_key_agreement(authn);
        let shared_secret = establish_shared_secret(&key_agreement);
        let new_pin_enc = encrypt_pin(shared_secret.as_slice(), new_pin);
        let mut old_pin_hash = pin_hash_prefix(old_pin).to_vec();
        let pin_hash_enc = encrypt_exact(shared_secret.as_slice(), &mut old_pin_hash);

        let mut pin_auth_input = new_pin_enc.clone();
        pin_auth_input.extend_from_slice(&pin_hash_enc);
        let pin_auth = hmac_left_16(shared_secret.as_slice(), &pin_auth_input);

        let platform_key = key_agreement_from_public(&shared_secret.platform_public);
        let request = client_pin_request(
            ctap2::client_pin::PinV1Subcommand::ChangePin,
            Some(&platform_key),
            Some(pin_auth.to_vec()),
            Some(new_pin_enc),
            Some(pin_hash_enc),
            None,
        );

        authn.call_ctap2(&Request::ClientPin(request)).map(|_| ())
    }

    pub fn get_pin_token(authn: &mut dyn TestAuthenticator, pin: &str) -> Self {
        Self::try_get_pin_token(authn, pin).expect("GetPinToken should succeed")
    }

    pub fn try_get_pin_token(
        authn: &mut dyn TestAuthenticator,
        pin: &str,
    ) -> Result<Self, ctap2::Error> {
        let key_agreement = get_authenticator_key_agreement(authn);
        let shared_secret = establish_shared_secret(&key_agreement);
        let mut pin_hash = pin_hash_prefix(pin).to_vec();
        let pin_hash_enc = encrypt_exact(shared_secret.as_slice(), &mut pin_hash);

        let platform_key = key_agreement_from_public(&shared_secret.platform_public);
        let request = client_pin_request(
            ctap2::client_pin::PinV1Subcommand::GetPinToken,
            Some(&platform_key),
            None,
            None,
            Some(pin_hash_enc),
            None,
        );

        let response = authn.call_ctap2(&Request::ClientPin(request))?;
        let encrypted_token = match response {
            Response::ClientPin(response) => response
                .pin_token
                .expect("GetPinToken should return pinToken"),
            other => panic!("Expected ClientPin response, got {:?}", other),
        };

        let mut encrypted = encrypted_token.as_slice().to_vec();
        let token = decrypt_exact(shared_secret.as_slice(), &mut encrypted);

        Ok(Self { token })
    }

    /// PIN token usable for credential-management (via `GetPinUvAuthTokenUsingPinWithPermissions`).
    ///
    /// `GetPinToken` only grants `MAKE_CREDENTIAL` and `GET_ASSERTION`; cred-mgmt requires
    /// [`ctap2::client_pin::Permissions::CREDENTIAL_MANAGEMENT`].
    pub fn get_pin_token_with_cred_mgmt(authn: &mut dyn TestAuthenticator, pin: &str) -> Self {
        Self::try_get_pin_token_with_permissions(
            authn,
            pin,
            ctap2::client_pin::Permissions::MAKE_CREDENTIAL
                | ctap2::client_pin::Permissions::GET_ASSERTION
                | ctap2::client_pin::Permissions::CREDENTIAL_MANAGEMENT,
        )
        .expect("GetPinUvAuthTokenUsingPinWithPermissions should succeed")
    }

    pub fn try_get_pin_token_with_permissions(
        authn: &mut dyn TestAuthenticator,
        pin: &str,
        permissions: ctap2::client_pin::Permissions,
    ) -> Result<Self, ctap2::Error> {
        let key_agreement = get_authenticator_key_agreement(authn);
        let shared_secret = establish_shared_secret(&key_agreement);
        let mut pin_hash = pin_hash_prefix(pin).to_vec();
        let pin_hash_enc = encrypt_exact(shared_secret.as_slice(), &mut pin_hash);

        let platform_key = key_agreement_from_public(&shared_secret.platform_public);
        let request = client_pin_request(
            ctap2::client_pin::PinV1Subcommand::GetPinUvAuthTokenUsingPinWithPermissions,
            Some(&platform_key),
            None,
            None,
            Some(pin_hash_enc),
            Some(permissions.bits()),
        );

        let response = authn.call_ctap2(&Request::ClientPin(request))?;
        let encrypted_token = match response {
            Response::ClientPin(response) => response
                .pin_token
                .expect("GetPinUvAuthTokenUsingPinWithPermissions should return pinToken"),
            other => panic!("Expected ClientPin response, got {:?}", other),
        };

        let mut encrypted = encrypted_token.as_slice().to_vec();
        let token = decrypt_exact(shared_secret.as_slice(), &mut encrypted);

        Ok(Self { token })
    }

    pub fn pin_auth_for_client_data_hash(&self, client_data_hash: &[u8]) -> [u8; 16] {
        hmac_left_16(&self.token, client_data_hash)
    }

    pub fn pin_auth_for_credential_management(
        &self,
        sub_command: ctap2::credential_management::Subcommand,
        params: Option<&Value>,
    ) -> [u8; 16] {
        let mut data = vec![sub_command as u8];
        match sub_command {
            ctap2::credential_management::Subcommand::EnumerateCredentialsBegin
            | ctap2::credential_management::Subcommand::DeleteCredential => {
                let params =
                    params.expect("credential management params required for this subcommand");
                let encoded =
                    serde_cbor::to_vec(params).expect("serialize credential management params");
                data.extend_from_slice(&encoded);
            }
            _ => {}
        }
        hmac_left_16(&self.token, &data)
    }

    pub fn protocol(&self) -> u8 {
        1
    }
}

pub fn get_retries(authn: &mut dyn TestAuthenticator) -> u8 {
    let request = client_pin_request(
        ctap2::client_pin::PinV1Subcommand::GetRetries,
        None,
        None,
        None,
        None,
        None,
    );
    match authn
        .call_ctap2(&Request::ClientPin(request))
        .expect("GetRetries should succeed")
    {
        Response::ClientPin(response) => {
            response.retries.expect("GetRetries should return retries")
        }
        other => panic!("Expected ClientPin response, got {:?}", other),
    }
}

pub struct SharedSecret {
    pub platform_public: PublicKey,
    pub bytes: [u8; 32],
}

impl SharedSecret {
    pub fn as_slice(&self) -> &[u8; 32] {
        &self.bytes
    }
}

pub fn get_authenticator_key_agreement(
    authn: &mut dyn TestAuthenticator,
) -> EcdhEsHkdf256PublicKey {
    let request = client_pin_request(
        ctap2::client_pin::PinV1Subcommand::GetKeyAgreement,
        None,
        None,
        None,
        None,
        None,
    );
    match authn
        .call_ctap2(&Request::ClientPin(request))
        .expect("GetKeyAgreement should succeed")
    {
        Response::ClientPin(response) => response
            .key_agreement
            .expect("GetKeyAgreement should return a key"),
        other => panic!("Expected ClientPin response, got {:?}", other),
    }
}

pub fn establish_shared_secret(authenticator_key: &EcdhEsHkdf256PublicKey) -> SharedSecret {
    let secret_key = SecretKey::random(&mut OsRng);
    let platform_public = secret_key.public_key();
    let peer_public =
        PublicKey::from_sec1_bytes(p256_public_key_bytes(authenticator_key).as_slice())
            .expect("valid authenticator key agreement point");
    let shared = diffie_hellman(secret_key.to_nonzero_scalar(), peer_public.as_affine());
    let shared_secret = Sha256::digest(shared.raw_secret_bytes());

    SharedSecret {
        platform_public,
        bytes: shared_secret.into(),
    }
}

pub fn key_agreement_from_public(public_key: &PublicKey) -> EcdhEsHkdf256PublicKey {
    let encoded = public_key.to_encoded_point(false);
    EcdhEsHkdf256PublicKey {
        x: ctap_types::Bytes::try_from(encoded.x().expect("uncompressed x").as_slice()).unwrap(),
        y: ctap_types::Bytes::try_from(encoded.y().expect("uncompressed y").as_slice()).unwrap(),
    }
}

fn p256_public_key_bytes(public_key: &EcdhEsHkdf256PublicKey) -> [u8; 65] {
    let mut encoded = [0u8; 65];
    encoded[0] = 0x04;
    encoded[1..33].copy_from_slice(public_key.x.as_slice());
    encoded[33..65].copy_from_slice(public_key.y.as_slice());
    encoded
}

fn encrypt_pin(shared_secret: &[u8], pin: &str) -> Vec<u8> {
    let mut plaintext = vec![0u8; 64];
    plaintext[..pin.len()].copy_from_slice(pin.as_bytes());
    encrypt_exact(shared_secret, &mut plaintext)
}

pub fn encrypt_exact(shared_secret: &[u8], plaintext: &mut [u8]) -> Vec<u8> {
    let iv = [0u8; 16];
    let mut buffer = plaintext.to_vec();
    let len = buffer.len();
    Aes256CbcEnc::new_from_slices(shared_secret, &iv)
        .unwrap()
        .encrypt_padded_mut::<NoPadding>(&mut buffer, len)
        .unwrap();
    buffer
}

pub fn decrypt_exact(shared_secret: &[u8], ciphertext: &mut [u8]) -> Vec<u8> {
    let iv = [0u8; 16];
    Aes256CbcDec::new_from_slices(shared_secret, &iv)
        .unwrap()
        .decrypt_padded_mut::<NoPadding>(ciphertext)
        .unwrap()
        .to_vec()
}

pub fn hmac_left_16(key: &[u8], data: &[u8]) -> [u8; 16] {
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(data);
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&mac.finalize().into_bytes()[..16]);
    tag
}

fn pin_hash_prefix(pin: &str) -> [u8; 16] {
    let mut prefix = [0u8; 16];
    prefix.copy_from_slice(&Sha256::digest(pin.as_bytes())[..16]);
    prefix
}
