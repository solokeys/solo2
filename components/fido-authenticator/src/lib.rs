 #![cfg_attr(not(test), no_std)]

use core::convert::{TryFrom, TryInto};

#[macro_use]
extern crate delog;
generate_macros!();

use trussed::{
    client, syscall, try_syscall,
    Client as TrussedClient,
    types::{
        KeySerialization,
        Mechanism,
        MediumData,
        Message,
        ObjectHandle,
        SignatureSerialization,
        Location,
    },
};

use ctap_types::{
    Bytes, Bytes32, consts, String, Vec,
    // cose::EcdhEsHkdf256PublicKey as CoseEcdhEsHkdf256PublicKey,
    // cose::PublicKey as CosePublicKey,
    operation::VendorOperation,
    // rpc::CtapInterchange,
    // authenticator::ctap1,
    authenticator::{ctap2, Error, Request, Response},
    ctap1::{
        self,
        Command as U2fCommand,
        Response as U2fResponse,
        Result as U2fResult,
        Error as U2fError,
    },
};

use littlefs2::path::{Path, PathBuf};

pub mod credential_management;
pub mod state;
pub mod constants;

use state::{
    MinCredentialHeap,
    TimestampPath,
};

// EWW.. this is a bit unsafe isn't it
fn format_hex(data: &[u8], mut buffer: &mut [u8]) {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    for byte in data.iter() {
        buffer[0] = HEX_CHARS[(byte >> 4) as usize];
        buffer[1] = HEX_CHARS[(byte & 0xf) as usize];
        buffer = &mut buffer[2..];
    }
}

fn rp_rk_dir(rp_id_hash: &Bytes<consts::U32>) -> PathBuf {
    // uses only first 8 bytes of hash, which should be "good enough"
    let mut hex = [b'0'; 16];
    format_hex(&rp_id_hash[..8], &mut hex);

    let mut dir = PathBuf::from(b"rk");
    dir.push(&PathBuf::from(&hex));

    dir
}

fn rk_path(rp_id_hash: &Bytes<consts::U32>, credential_id_hash: &Bytes<consts::U32>) -> PathBuf {
    let mut path = rp_rk_dir(rp_id_hash);

    let mut hex = [0u8; 16];
    format_hex(&credential_id_hash[..8], &mut hex);
    path.push(&PathBuf::from(&hex));

    path
}

pub mod credential;
pub use credential::*;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum SupportedAlgorithm {
    P256 = -7,
    Ed25519 = -8,
    Totp = -9,
}

impl core::convert::TryFrom<i32> for SupportedAlgorithm {
    type Error = Error;
    fn try_from(alg: i32) -> Result<Self> {
        Ok(match alg {
            -7 => SupportedAlgorithm::P256,
            -8 => SupportedAlgorithm::Ed25519,
            -9 => SupportedAlgorithm::Totp,
            _ => return Err(Error::UnsupportedAlgorithm),
        })
    }
}

/// Idea is to maybe send a request over a queue,
/// and return upon button press.
/// TODO: Do we need a timeout?
pub trait UserPresence: Copy {
    fn user_present<T: TrussedClient>(self, trussed: &mut T, timeout_milliseconds: u32) -> bool;
}

#[derive(Copy, Clone)]
pub struct SilentAuthenticator {}

impl UserPresence for SilentAuthenticator {
    fn user_present<T: TrussedClient>(self, _: &mut T, _:u32) -> bool {
        true
    }
}

#[derive(Copy, Clone)]
pub struct NonSilentAuthenticator {}

impl UserPresence for NonSilentAuthenticator {
    fn user_present<T: TrussedClient>(self, trussed: &mut T, timeout_milliseconds: u32) -> bool {
        let result = syscall!(trussed.confirm_user_present(timeout_milliseconds)).result;
        result.is_ok()
    }
}

fn cbor_serialize_message<T: serde::Serialize>(object: &T) -> core::result::Result<Message, ctap_types::serde::Error> {
    Ok(trussed::cbor_serialize_bytes(object)?)
}

pub struct Authenticator<UP, T>
where UP: UserPresence,
{
    trussed: T,
    state: state::State,
    up: UP,
}

impl<UP, T> Authenticator<UP, T>
where UP: UserPresence,
      T: client::Client
       + client::P256
       + client::Chacha8Poly1305
       + client::Aes256Cbc
       + client::Sha256
       + client::HmacSha256
       + client::Ed255
       + client::Totp
       // + TrussedClient
{
    pub fn new(trussed: T, up: UP) -> Self {

        let state = state::State::new();
        let authenticator = Self { trussed, state, up };

        authenticator
    }

    pub fn call_u2f(&mut self, request: &U2fCommand) -> U2fResult<U2fResponse> {
        info!("called u2f");
        self.state.persistent.load_if_not_initialised(&mut self.trussed);

        let mut commitment = Bytes::<consts::U324>::new();

        match request {
            U2fCommand::Register(reg) => {

                if !self.up.user_present(&mut self.trussed, constants::U2F_UP_TIMEOUT) {
                    return Err(U2fError::ConditionsOfUseNotSatisfied);
                }
                // Generate a new P256 key pair.
                let private_key = syscall!(self.trussed.generate_p256_private_key(Location::Volatile)).key;
                let public_key = syscall!(self.trussed.derive_p256_public_key(private_key, Location::Volatile)).key;

                let serialized_cose_public_key = syscall!(self.trussed.serialize_p256_key(
                    public_key, KeySerialization::EcdhEsHkdf256
                )).serialized_key;
                let cose_key: ctap_types::cose::EcdhEsHkdf256PublicKey
                    = trussed::cbor_deserialize(&serialized_cose_public_key).unwrap();

                let wrapping_key = self.state.persistent.key_wrapping_key(&mut self.trussed)
                    .map_err(|_| U2fError::UnspecifiedCheckingError)?;
                debug!("wrapping u2f private key");
                let wrapped_key = syscall!(self.trussed.wrap_key_chacha8poly1305(
                    wrapping_key,
                    private_key,
                    &reg.app_id,
                )).wrapped_key;
                // debug!("wrapped_key = {:?}", &wrapped_key);

                let key = Key::WrappedKey(wrapped_key.try_to_bytes().map_err(|_| U2fError::UnspecifiedCheckingError)?);
                let nonce = syscall!(self.trussed.random_bytes(12)).bytes.as_slice().try_into().unwrap();

                let mut rp_id = heapless::String::new();

                // We do not know the rpId string in U2F.  Just using placeholder.
                rp_id.push_str("u2f").ok();
                let rp = ctap_types::webauthn::PublicKeyCredentialRpEntity{
                    id: rp_id,
                    name: None,
                    url: None,
                };

                let user = ctap_types::webauthn::PublicKeyCredentialUserEntity {
                    id: Bytes::try_from_slice(&[0u8; 8]).unwrap(),
                    icon: None,
                    name: None,
                    display_name: None,
                };

                let credential = Credential::new(
                    credential::CtapVersion::U2fV2,
                    &rp,
                    &user,

                    SupportedAlgorithm::P256 as i32,
                    key,
                    self.state.persistent.timestamp(&mut self.trussed).map_err(|_| U2fError::NotEnoughMemory)?,
                    None,
                    None,
                    nonce,
                );

                // info!("made credential {:?}", &credential);

                // 12.b generate credential ID { = AEAD(Serialize(Credential)) }
                let kek = self.state.persistent.key_encryption_key(&mut self.trussed).map_err(|_| U2fError::NotEnoughMemory)?;
                let credential_id = credential.id_using_hash(&mut self.trussed, kek, &reg.app_id).map_err(|_| U2fError::NotEnoughMemory)?;
                syscall!(self.trussed.delete(public_key));
                syscall!(self.trussed.delete(private_key));

                commitment.push(0).unwrap();     // reserve byte
                commitment.extend_from_slice(&reg.app_id).unwrap();
                commitment.extend_from_slice(&reg.challenge).unwrap();

                commitment.extend_from_slice(&credential_id.0).unwrap();

                commitment.push(0x04).unwrap();  // public key uncompressed byte
                commitment.extend_from_slice(&cose_key.x).unwrap();
                commitment.extend_from_slice(&cose_key.y).unwrap();

                let attestation = self.state.identity.attestation(&mut self.trussed);

                let (signature, cert) = match attestation {
                    (Some((key, cert)), _aaguid) => {
                        info!("aaguid: {}", hex_str!(&_aaguid));
                        (
                            syscall!(
                                self.trussed.sign(Mechanism::P256,
                                key,
                                &commitment,
                                SignatureSerialization::Asn1Der
                            )).signature.to_bytes(),
                            cert
                        )
                    },
                    _ => {
                        info!("Not provisioned with attestation key!");
                        return Err(U2fError::KeyReferenceNotFound);
                    }
                };


                Ok(U2fResponse::Register(ctap1::RegisterResponse::new(
                    0x05,
                    &cose_key,
                    &credential_id.0,
                    signature,
                    &cert,
                )))
            }
            U2fCommand::Authenticate(auth) => {

                let cred = Credential::try_from_bytes(self, &auth.app_id, &auth.key_handle);

                let user_presence_byte = match auth.control_byte {
                    ctap1::ControlByte::CheckOnly => {
                        // if the control byte is set to 0x07 by the FIDO Client,
                        // the U2F token is supposed to simply check whether the
                        // provided key handle was originally created by this token
                        return if cred.is_ok() {
                            Err(U2fError::ConditionsOfUseNotSatisfied)
                        } else {
                            Err(U2fError::IncorrectDataParameter)
                        };
                    },
                    ctap1::ControlByte::EnforceUserPresenceAndSign => {
                        if !self.up.user_present(&mut self.trussed, constants::U2F_UP_TIMEOUT) {
                            return Err(U2fError::ConditionsOfUseNotSatisfied);
                        }
                        0x01
                    },
                    ctap1::ControlByte::DontEnforceUserPresenceAndSign => 0x00,
                };

                let cred = cred.map_err(|_| U2fError::IncorrectDataParameter)?;

                let key = match &cred.key {
                    Key::WrappedKey(bytes) => {
                        let wrapping_key = self.state.persistent.key_wrapping_key(&mut self.trussed)
                            .map_err(|_| U2fError::IncorrectDataParameter)?;
                        let key_result = syscall!(self.trussed.unwrap_key_chacha8poly1305(
                            wrapping_key,
                            bytes,
                            b"",
                            Location::Volatile,
                        )).key;
                        match key_result {
                            Some(key) => {
                                info!("loaded u2f key!");
                                key
                            }
                            None => {
                                info!("issue with unwrapping credential id key");
                                return Err(U2fError::IncorrectDataParameter);
                            }
                        }
                    }
                    _ => return Err(U2fError::IncorrectDataParameter),
                };

                if cred.algorithm != -7 {
                    info!("Unexpected mechanism for u2f");
                    return Err(U2fError::IncorrectDataParameter);
                }

                let sig_count = self.state.persistent.timestamp(&mut self.trussed).
                    map_err(|_| U2fError::UnspecifiedNonpersistentExecutionError)?;

                commitment.extend_from_slice(&auth.app_id).unwrap();
                commitment.push(user_presence_byte).unwrap();
                commitment.extend_from_slice(&sig_count.to_be_bytes()).unwrap();
                commitment.extend_from_slice(&auth.challenge).unwrap();

                let signature = syscall!(
                    self.trussed.sign(Mechanism::P256,
                    key,
                    &commitment,
                    SignatureSerialization::Asn1Der
                )).signature.to_bytes();

                Ok(U2fResponse::Authenticate(ctap1::AuthenticateResponse::new(
                    user_presence_byte,
                    sig_count,
                    signature,
                )))

            }
            U2fCommand::Version => {
                // "U2F_V2"
                Ok(U2fResponse::Version([0x55, 0x32, 0x46, 0x5f, 0x56, 0x32]))
            }
        }

    }

    pub fn call(&mut self, request: &Request) -> Result<Response> {
        // if let Some(request) = self.interchange.take_request() {
            // debug!("request: {:?}", &request);
            self.state.persistent.load_if_not_initialised(&mut self.trussed);

            match request {
                Request::Ctap2(request) => {
                    match request {

                        // 0x4
                        ctap2::Request::GetInfo => {
                            debug!("GI");
                            let response = self.get_info();
                            Ok(Response::Ctap2(ctap2::Response::GetInfo(response)))
                        }

                        // 0x2
                        ctap2::Request::MakeCredential(parameters) => {
                            debug!("MC request");
                            let response = self.make_credential(&parameters);
                            match response {
                                Ok(response) => Ok(Response::Ctap2(ctap2::Response::MakeCredential(response))),
                                Err(error) => Err(error)
                            }
                        }

                        // 0x1
                        ctap2::Request::GetAssertion(parameters) => {
                            debug!("GA request");
                            let response = self.get_assertion(&parameters);
                            match response {
                                Ok(response) => Ok(Response::Ctap2(ctap2::Response::GetAssertion(response))),
                                Err(error) => Err(error)
                            }
                        }

                        // 0x8
                        ctap2::Request::GetNextAssertion => {
                            debug!("GNA request");
                            let response = self.get_next_assertion();
                            match response {
                                Ok(response) => Ok(Response::Ctap2(ctap2::Response::GetNextAssertion(response))),
                                Err(error) => Err(error)
                            }
                        }

                        // 0x7
                        ctap2::Request::Reset => {
                            debug!("GA request");
                            let response = self.reset();
                            match response {
                                Ok(()) => Ok(Response::Ctap2(ctap2::Response::Reset)),
                                Err(error) => Err(error)
                            }
                        }


                        // 0x6
                        ctap2::Request::ClientPin(parameters) => {
                            debug!("CP request");
                            let response = self.client_pin(&parameters);
                            match response {
                                Ok(response) => Ok(Response::Ctap2(ctap2::Response::ClientPin(response))),
                                Err(error) => Err(error)
                            }
                        }

                        // 0xA
                        ctap2::Request::CredentialManagement(parameters) => {
                            debug!("CM request");
                            let response = self.credential_management(&parameters);
                            match response {
                                Ok(response) => {
                                    // let mut buf = [0u8; 512];
                                    // info!("{:?}", ctap_types::serde::cbor_serialize(&response, &mut buf));
                                    Ok(Response::Ctap2(ctap2::Response::CredentialManagement(response)))
                                }
                                Err(error) => Err(error)
                            }
                        }


                        ctap2::Request::Vendor(op) => {
                            debug!("Vendor request");
                            let response = self.vendor(*op);
                            match response {
                                Ok(()) => Ok(Response::Ctap2(ctap2::Response::Vendor)),
                                Err(error) => Err(error)
                            }
                        }

                        // _ => {
                        //     // debug!("not implemented: {:?}", &request);
                        //     debug!("request not implemented");
                        //     self.interchange.respond(Err(Error::InvalidCommand)).expect("internal error");
                        // }
                    }
                }
                Request::Ctap1(_request) => {
                    debug!("ctap1 not implemented: {:?}", &_request);
                    Err(Error::InvalidCommand)
                }
            }
        // }
    }

    fn client_pin(&mut self, parameters: &ctap2::client_pin::Parameters) -> Result<ctap2::client_pin::Response> {
        use ctap2::client_pin::PinV1Subcommand as Subcommand;
        debug!("processing CP");
        // info!("{:?}", parameters);

        if parameters.pin_protocol != 1{
            return Err(Error::InvalidParameter);
        }

        Ok(match parameters.sub_command {

            Subcommand::GetRetries => {
                debug!("processing CP.GR");

                ctap2::client_pin::Response {
                    key_agreement: None,
                    pin_token: None,
                    retries: Some(self.state.persistent.retries()),
                }
            }

            Subcommand::GetKeyAgreement => {
                debug!("processing CP.GKA");

                let private_key = self.state.runtime.key_agreement_key(&mut self.trussed);
                let public_key = syscall!(self.trussed.derive_p256_public_key(private_key, Location::Volatile)).key;
                let serialized_cose_key = syscall!(self.trussed.serialize_key(
                    Mechanism::P256, public_key.clone(), KeySerialization::EcdhEsHkdf256)).serialized_key;
                let cose_key = trussed::cbor_deserialize(&serialized_cose_key).unwrap();

                syscall!(self.trussed.delete(public_key));

                ctap2::client_pin::Response {
                    key_agreement: cose_key,
                    pin_token: None,
                    retries: None,
                }
            }

            Subcommand::SetPin => {
                debug!("processing CP.SP");
                // 1. check mandatory parameters
                let platform_kek = match parameters.key_agreement.as_ref() {
                    Some(key) => key,
                    None => { return Err(Error::MissingParameter); }
                };
                let new_pin_enc = match parameters.new_pin_enc.as_ref() {
                    Some(pin) => pin,
                    None => { return Err(Error::MissingParameter); }
                };
                let pin_auth = match parameters.pin_auth.as_ref() {
                    Some(auth) => auth,
                    None => { return Err(Error::MissingParameter); }
                };

                // 2. is pin already set
                if self.state.persistent.pin_is_set() {
                    return Err(Error::NotAllowed);
                }

                // 3. generate shared secret
                let shared_secret = self.state.runtime.generate_shared_secret(&mut self.trussed, platform_kek)?;

                // TODO: there are moar early returns!!
                // - implement Drop?
                // - do garbage collection outside of this?

                // 4. verify pinAuth
                self.verify_pin_auth(shared_secret, new_pin_enc, pin_auth)?;

                // 5. decrypt and verify new PIN
                let new_pin = self.decrypt_pin_check_length(shared_secret, new_pin_enc)?;

                syscall!(self.trussed.delete(shared_secret));

                // 6. store LEFT(SHA-256(newPin), 16), set retries to 8
                self.hash_store_pin(&new_pin)?;
                self.state.reset_retries(&mut self.trussed).map_err(|_| Error::Other)?;

                ctap2::client_pin::Response {
                    key_agreement: None,
                    pin_token: None,
                    retries: None,
                }
            }

            Subcommand::ChangePin => {
                debug!("processing CP.CP");

                // 1. check mandatory parameters
                let platform_kek = match parameters.key_agreement.as_ref() {
                    Some(key) => key,
                    None => { return Err(Error::MissingParameter); }
                };
                let pin_hash_enc = match parameters.pin_hash_enc.as_ref() {
                    Some(hash) => hash,
                    None => { return Err(Error::MissingParameter); }
                };
                let new_pin_enc = match parameters.new_pin_enc.as_ref() {
                    Some(pin) => pin,
                    None => { return Err(Error::MissingParameter); }
                };
                let pin_auth = match parameters.pin_auth.as_ref() {
                    Some(auth) => auth,
                    None => { return Err(Error::MissingParameter); }
                };

                // 2. fail if no retries left
                self.state.pin_blocked()?;

                // 3. generate shared secret
                let shared_secret = self.state.runtime.generate_shared_secret(&mut self.trussed, platform_kek)?;

                // 4. verify pinAuth
                let mut data = MediumData::new();
                data.extend_from_slice(new_pin_enc).map_err(|_| Error::InvalidParameter)?;
                data.extend_from_slice(pin_hash_enc).map_err(|_| Error::InvalidParameter)?;
                self.verify_pin_auth(shared_secret, &data, pin_auth)?;

                // 5. decrement retries
                self.state.decrement_retries(&mut self.trussed)?;

                // 6. decrypt pinHashEnc, compare with stored
                self.decrypt_pin_hash_and_maybe_escalate(shared_secret, &pin_hash_enc)?;

                // 7. reset retries
                self.state.reset_retries(&mut self.trussed)?;

                // 8. decrypt and verify new PIN
                let new_pin = self.decrypt_pin_check_length(shared_secret, new_pin_enc)?;

                syscall!(self.trussed.delete(shared_secret));

                // 9. store hashed PIN
                self.hash_store_pin(&new_pin)?;

                ctap2::client_pin::Response {
                    key_agreement: None,
                    pin_token: None,
                    retries: None,
                }
            }

            Subcommand::GetPinToken => {
                debug!("processing CP.GPT");

                // 1. check mandatory parameters
                let platform_kek = match parameters.key_agreement.as_ref() {
                    Some(key) => key,
                    None => { return Err(Error::MissingParameter); }
                };
                let pin_hash_enc = match parameters.pin_hash_enc.as_ref() {
                    Some(hash) => hash,
                    None => { return Err(Error::MissingParameter); }
                };

                // 2. fail if no retries left
                self.state.pin_blocked()?;

                // 3. generate shared secret
                let shared_secret = self.state.runtime.generate_shared_secret(&mut self.trussed, platform_kek)?;

                // 4. decrement retires
                self.state.decrement_retries(&mut self.trussed)?;

                // 5. decrypt and verify pinHashEnc
                self.decrypt_pin_hash_and_maybe_escalate(shared_secret, &pin_hash_enc)?;

                // 6. reset retries
                self.state.reset_retries(&mut self.trussed)?;

                // 7. return encrypted pinToken
                let pin_token = self.state.runtime.pin_token(&mut self.trussed);
                debug!("wrapping pin token");
                // info!("exists? {}", syscall!(self.trussed.exists(shared_secret)).exists);
                let pin_token_enc = syscall!(self.trussed.wrap_key_aes256cbc(shared_secret, pin_token)).wrapped_key;

                syscall!(self.trussed.delete(shared_secret));

                // ble...
                if pin_token_enc.len() != 16 {
                    return Err(Error::Other);
                }
                let pin_token_enc_32 = Bytes::try_from_slice(&pin_token_enc).unwrap();

                ctap2::client_pin::Response {
                    key_agreement: None,
                    pin_token: Some(pin_token_enc_32),
                    retries: None,
                }
            }

        })
    }

    fn decrypt_pin_hash_and_maybe_escalate(&mut self, shared_secret: ObjectHandle, pin_hash_enc: &Bytes<consts::U64>)
        -> Result<()>
    {
        let pin_hash = syscall!(self.trussed.decrypt_aes256cbc(
            shared_secret, pin_hash_enc)).plaintext.ok_or(Error::Other)?;

        let stored_pin_hash = match self.state.persistent.pin_hash() {
            Some(hash) => hash,
            None => { return Err(Error::PinNotSet); }
        };

        if &pin_hash != &stored_pin_hash {
            // I) generate new KEK
            self.state.runtime.rotate_key_agreement_key(&mut self.trussed);
            if self.state.persistent.retries() == 0 {
                return Err(Error::PinBlocked);
            }
            if self.state.persistent.pin_blocked() {
                return Err(Error::PinAuthBlocked);
            }
            return Err(Error::PinInvalid);
        }

        Ok(())
    }

    fn hash_store_pin(&mut self, pin: &Message) -> Result<()> {
        let pin_hash_32 = syscall!(self.trussed.hash_sha256(&pin)).hash;
        let pin_hash: [u8; 16] = pin_hash_32[..16].try_into().unwrap();
        self.state.persistent.set_pin_hash(&mut self.trussed, pin_hash).unwrap();

        Ok(())
    }

    fn decrypt_pin_check_length(&mut self, shared_secret: ObjectHandle, pin_enc: &[u8]) -> Result<Message> {
        // pin is expected to be filled with null bytes to length at least 64
        if pin_enc.len() < 64 {
            // correct error?
            return Err(Error::PinPolicyViolation);
        }

        let mut pin = syscall!(self.trussed.decrypt_aes256cbc(
            shared_secret, &pin_enc)).plaintext.ok_or(Error::Other)?;

        // // temp
        // let pin_length = pin.iter().position(|&b| b == b'\0').unwrap_or(pin.len());
        // info!("pin.len() = {}, pin_length = {}, = {:?}",
        //           pin.len(), pin_length, &pin);
        // chop off null bytes
        let pin_length = pin.iter().position(|&b| b == b'\0').unwrap_or(pin.len());
        if pin_length < 4 || pin_length >= 64 {
            return Err(Error::PinPolicyViolation);
        }

        pin.resize_default(pin_length).unwrap();

        Ok(pin)
    }


    // fn verify_pin(&mut self, pin_auth: &Bytes<consts::U16>, client_data_hash: &Bytes<consts::U32>) -> bool {
    fn verify_pin(&mut self, pin_auth: &[u8; 16], data: &[u8]) -> Result<()> {
        let key = self.state.runtime.pin_token(&mut self.trussed);
        let tag = syscall!(self.trussed.sign_hmacsha256(key, data)).signature;
        if pin_auth == &tag[..16] {
            Ok(())
        } else {
            Err(Error::PinAuthInvalid)
        }
    }

    fn verify_pin_auth(&mut self, shared_secret: ObjectHandle, data: &[u8], pin_auth: &Bytes<consts::U16>)
        -> Result<()>
    {
        let expected_pin_auth = syscall!(self.trussed.sign_hmacsha256(shared_secret, data)).signature;

        if &expected_pin_auth[..16] == &pin_auth[..] {
            Ok(())
        } else {
            Err(Error::PinAuthInvalid)
        }
    }

    // fn verify_pin_auth_using_token(&mut self, data: &[u8], pin_auth: &Bytes<consts::U16>)
    fn verify_pin_auth_using_token(
        &mut self,
        parameters: &ctap2::credential_management::Parameters
    ) -> Result<()> {

        // info!("CM params: {:?}", parameters);
        use ctap2::credential_management::Subcommand;
        match parameters.sub_command {
            // are we Haskell yet lol
            sub_command @ Subcommand::GetCredsMetadata |
            sub_command @ Subcommand::EnumerateRpsBegin |
            sub_command @ Subcommand::EnumerateCredentialsBegin |
            sub_command @ Subcommand::DeleteCredential => {

                // check pinProtocol
                let pin_protocol = parameters
                    // .sub_command_params.as_ref().ok_or(Error::MissingParameter)?
                    .pin_protocol.ok_or(Error::MissingParameter)?;
                if pin_protocol != 1 {
                    return Err(Error::InvalidParameter);
                }

                // check pinAuth
                let pin_token = self.state.runtime.pin_token(&mut self.trussed);
                type BufSize = <
                        heapless::consts::U256 as core::ops::Add<ctap_types::sizes::MAX_CREDENTIAL_ID_LENGTH>
                    >::Output;
                let mut data: Bytes<BufSize> =
                    Bytes::try_from_slice(&[sub_command as u8]).unwrap();
                let len = 1 + match sub_command {
                    Subcommand::EnumerateCredentialsBegin |
                    Subcommand::DeleteCredential => {
                        data.resize_to_capacity();
                        // ble, need to reserialize
                        ctap_types::serde::cbor_serialize(
                            &parameters.sub_command_params
                            .as_ref()
                            .ok_or(Error::MissingParameter)?,
                            &mut data[1..],
                        ).map_err(|_| Error::LimitExceeded)?.len()
                    }
                    _ => 0,
                };

                // info!("input to hmacsha256: {:?}", &data[..len]);
                let expected_pin_auth = syscall!(self.trussed.sign_hmacsha256(
                    pin_token,
                    &data[..len],
                )).signature;

                let pin_auth = parameters
                    .pin_auth.as_ref().ok_or(Error::MissingParameter)?;

                if &expected_pin_auth[..16] == &pin_auth[..] {
                    info!("passed pinauth");
                    Ok(())
                } else {
                    info!("failed pinauth!");
                    self.state.decrement_retries(&mut self.trussed)?;
                    let maybe_blocked = self.state.pin_blocked();
                    if maybe_blocked.is_err() {
                        info!("blocked");
                        maybe_blocked
                    } else {
                        info!("pinAuthInvalid");
                        Err(Error::PinAuthInvalid)
                    }

                }
            }

            _ => Ok(()),
        }
    }

    /// Returns whether UV was performed.
    fn pin_prechecks(&mut self,
        options: &Option<ctap2::AuthenticatorOptions>,
        pin_auth: &Option<ctap2::PinAuth>,
        pin_protocol: &Option<u32>,
        data: &[u8],
    )
        -> Result<bool>
    {
        // 1. pinAuth zero length -> wait for user touch, then
        // return PinNotSet if not set, PinInvalid if set
        //
        // the idea is for multi-authnr scenario where platform
        // wants to enforce PIN and needs to figure out which authnrs support PIN
        if let Some(pin_auth) = pin_auth.as_ref() {
            if pin_auth.len() == 0 {
                if !self.up.user_present(&mut self.trussed, constants::FIDO2_UP_TIMEOUT) {
                    return Err(Error::OperationDenied);
                }
                if !self.state.persistent.pin_is_set() {
                    return Err(Error::PinNotSet);
                } else {
                    return Err(Error::PinAuthInvalid);
                }
            }
        }

        // 2. check PIN protocol is 1 if pinAuth was sent
        if let Some(ref _pin_auth) = pin_auth {
            if let Some(1) = pin_protocol {
            } else {
                return Err(Error::PinAuthInvalid);
            }
        }

        // 3. if no PIN is set (we have no other form of UV),
        // and platform sent `uv` or `pinAuth`, return InvalidOption
        if !self.state.persistent.pin_is_set() {
            if let Some(ref options) = &options {
                if Some(true) == options.uv {
                    return Err(Error::InvalidOption);
                }
            }
            if pin_auth.is_some() {
                return Err(Error::InvalidOption);
            }
        }

        // 4. If authenticator is protected by som form of user verification, do it
        //
        // TODO: Should we should fail if `uv` is passed?
        // Current thinking: no
        if self.state.persistent.pin_is_set() {

            // let mut uv_performed = false;
            if let Some(ref pin_auth) = pin_auth {
                if pin_auth.len() != 16 {
                    return Err(Error::InvalidParameter);
                }
                // seems a bit redundant to check here in light of 2.
                // I guess the CTAP spec writers aren't implementers :D
                if let Some(1) = pin_protocol {
                    // 5. if pinAuth is present and pinProtocol = 1, verify
                    // success --> set uv = 1
                    // error --> PinAuthInvalid
                    self.verify_pin(
                        // unwrap panic ruled out above
                        pin_auth.as_slice().try_into().unwrap(),
                        data,
                    )?;

                    return Ok(true);

                } else {
                    // 7. pinAuth present + pinProtocol != 1 --> error PinAuthInvalid
                    return Err(Error::PinAuthInvalid);
                }

            } else {
                // 6. pinAuth not present + clientPin set --> error PinRequired
                if self.state.persistent.pin_is_set() {
                    return Err(Error::PinRequired);
                }
            }
        }

        Ok(false)
    }

    /// If allow_list is some, select the first one that is usable,
    /// and return some(it).
    ///
    /// If allow_list is none, pull applicable credentials, store
    /// in state's credential_heap, and return none
    #[inline(never)]
    fn locate_credentials(
        &mut self, rp_id_hash: &Bytes32,
        allow_list: &Option<ctap2::get_assertion::AllowList>,
        uv_performed: bool,
    )
        -> Result<()>
    {
        // validate allowList
        let mut allow_list_len = 0;
        let allowed_credentials = if let Some(allow_list) = allow_list.as_ref() {
            allow_list_len = allow_list.len();
            allow_list.into_iter()
                // discard not properly serialized encrypted credentials
                .filter_map(|credential_descriptor| {
                    info!(
                        "GA try from cred id: {}",
                        hex_str!(&credential_descriptor.id),
                    );
                    let cred_maybe = Credential::try_from(
                        self, rp_id_hash, credential_descriptor)
                        .ok();
                    info!("cred_maybe: {:?}", &cred_maybe);
                    cred_maybe
                } )
                .collect()
        } else {
            CredentialList::new()
        };

        let mut min_heap = MinCredentialHeap::new();

        let allowed_credentials_passed = allowed_credentials.len() > 0;

        if allowed_credentials_passed {
            // "If an allowList is present and is non-empty,
            // locate all denoted credentials present on this authenticator
            // and bound to the specified rpId."
            debug!("allowedList passed with {} creds", allowed_credentials.len());
            let mut applicable_credentials: CredentialList = allowed_credentials
                .into_iter()
                .filter(|credential| match credential.key.clone() {
                    // TODO: should check if wrapped key is valid AEAD
                    // On the other hand, we already decrypted a valid AEAD
                    Key::WrappedKey(_) => true,
                    Key::ResidentKey(key) => {
                        debug!("checking if ResidentKey {:?} exists", &key);
                        match credential.algorithm {
                            -7 => syscall!(self.trussed.exists(Mechanism::P256, key)).exists,
                            -8 => syscall!(self.trussed.exists(Mechanism::Ed255, key)).exists,
                            -9 => {
                                let exists = syscall!(self.trussed.exists(Mechanism::Totp, key)).exists;
                                info!("found it");
                                exists
                            }
                            _ => false,
                        }
                    }
                })
                .filter(|credential| {
                    use credential::CredentialProtectionPolicy as Policy;
                    debug!("CredentialProtectionPolicy {:?}", &credential.cred_protect);
                    match credential.cred_protect {
                        None | Some(Policy::Optional) => true,
                        Some(Policy::OptionalWithCredentialIdList) => allowed_credentials_passed || uv_performed,
                        Some(Policy::Required) => uv_performed,

                    }
                })
                .collect();
            while applicable_credentials.len() > 0 {
                // Store all other applicable credentials in volatile storage and add to our
                // credential heap.
                let credential = applicable_credentials.pop().unwrap();
                let serialized = credential.serialize()?;

                let mut path = [b'0', b'0'];
                format_hex(&[applicable_credentials.len() as u8], &mut path);
                let path = PathBuf::from(&path);
                // let kek = self.state.persistent.key_encryption_key(&mut self.trussed)?;
                // let id = credential.id_using_hash(&mut self.trussed, kek, rp_id_hash)?;
                // let credential_id_hash = self.hash(&id.0.as_ref());

                // let path = rk_path(&rp_id_hash, &credential_id_hash);


                let timestamp_path = TimestampPath {
                    timestamp: credential.creation_time,
                    path: path.clone(),
                    location: Location::Volatile,
                };


                info!("added volatile cred: {:?}", &timestamp_path);
                info!("{}",hex_str!(&serialized));


                try_syscall!(self.trussed.write_file(
                    Location::Volatile,
                    path.clone(),
                    serialized,
                    None,
                )).map_err(|_| {
                    Error::KeyStoreFull
                })?;

                // attempt to read back
                let data = syscall!(self.trussed.read_file(
                    Location::Volatile,
                    timestamp_path.path.clone(),
                )).data;
                crate::Credential::deserialize(&data).unwrap();


                if min_heap.capacity() > min_heap.len() {
                    min_heap.push(timestamp_path).map_err(drop).unwrap();
                } else {
                    if timestamp_path.timestamp > min_heap.peek().unwrap().timestamp {
                        min_heap.pop().unwrap();
                        min_heap.push(timestamp_path).map_err(drop).unwrap();
                    }
                }

            }
        } else if allow_list_len == 0 {
            // If an allowList is not present,
            // locate all credentials that are present on this authenticator
            // and bound to the specified rpId; sorted by reverse creation time

            // let rp_id_hash = self.hash(rp_id.as_ref());

            //
            // So here's the idea:
            //
            // - credentials can be pretty big
            // - we declare N := MAX_CREDENTIAL_COUNT_IN_LIST in GetInfo
            // - potentially there are more RKs for a given RP (a bit academic ofc)
            //
            // - first, we use a min-heap to keep only the topN credentials:
            //   if our "next" one is larger/later than the min of the heap,
            //   pop this min and push ours
            //
            // - then, we use a max-heap to sort the remaining <=N credentials
            // - these then go into a CredentialList
            // - (we don't need to keep that around even)
            //
            debug!("no allowedList passed");

            // let mut credentials = CredentialList::new();

            let data = syscall!(self.trussed.read_dir_files_first(
                Location::Internal,
                rp_rk_dir(&rp_id_hash),
                None,
            )).data;

            let data = match data {
                Some(data) => data,
                None => return Err(Error::NoCredentials),
            };

            let credential = Credential::deserialize(&data).unwrap();

            use credential::CredentialProtectionPolicy as Policy;
            let keep = match credential.cred_protect {
                None | Some(Policy::Optional) => true,
                Some(Policy::OptionalWithCredentialIdList) => allowed_credentials_passed || uv_performed,
                Some(Policy::Required) => uv_performed,
            };

            let kek = self.state.persistent.key_encryption_key(&mut self.trussed)?;

            if keep {
                let id = credential.id_using_hash(&mut self.trussed, kek, rp_id_hash)?;
                let credential_id_hash = self.hash(&id.0.as_ref());

                let timestamp_path = TimestampPath {
                    timestamp: credential.creation_time,
                    path: rk_path(&rp_id_hash, &credential_id_hash),
                    location: Location::Internal,
                };

                min_heap.push(timestamp_path).map_err(drop).unwrap();
                // info!("first: {:?}", &self.hash(&id.0));
            }

            loop {
                let data = syscall!(self.trussed.read_dir_files_next()).data;
                let data = match data {
                    Some(data) => data,
                    None => break,
                };

                let credential = Credential::deserialize(&data).unwrap();

                let keep = match credential.cred_protect {
                    None | Some(Policy::Optional) => true,
                    Some(Policy::OptionalWithCredentialIdList) => allowed_credentials_passed || uv_performed,
                    Some(Policy::Required) => uv_performed,
                };

                if keep {

                    let id = credential.id_using_hash(&mut self.trussed, kek, rp_id_hash)?;
                    let credential_id_hash = self.hash(&id.0.as_ref());

                    let timestamp_path = TimestampPath {
                        timestamp: credential.creation_time,
                        path: rk_path(&rp_id_hash, &credential_id_hash),
                        location: Location::Internal,
                    };

                    if min_heap.capacity() > min_heap.len() {
                        min_heap.push(timestamp_path).map_err(drop).unwrap();
                    } else {
                        if timestamp_path.timestamp > min_heap.peek().unwrap().timestamp {
                            min_heap.pop().unwrap();
                            min_heap.push(timestamp_path).map_err(drop).unwrap();
                        }
                    }
                }
            }

        };

        // "If no applicable credentials were found, return CTAP2_ERR_NO_CREDENTIALS"
        if min_heap.is_empty() {
            return Err(Error::NoCredentials);
        }

        // now sort them
        self.state.runtime.free_credential_heap(&mut self.trussed);
        let max_heap = self.state.runtime.credential_heap();
        while !min_heap.is_empty() {
            max_heap.push(min_heap.pop().unwrap()).map_err(drop).unwrap();
        }

        Ok(())
    }

    fn get_next_assertion(&mut self) -> Result<ctap2::get_assertion::Response> {
        // 1./2. don't remember / don't have left any credentials
        if self.state.runtime.credential_heap().is_empty() {
            return Err(Error::NotAllowed);
        }

        // 3. previous GA/GNA >30s ago -> discard stat
        // this is optional over NFC
        if false {
            self.state.runtime.free_credential_heap(&mut self.trussed);
            return Err(Error::NotAllowed);
        }

        // 4. select credential
        // let data = syscall!(self.trussed.read_file(
        //     timestamp_hash.location,
        //     timestamp_hash.path,
        // )).data;
        let credential = self.state.runtime.pop_credential_from_heap(&mut self.trussed);
        // Credential::deserialize(&data).unwrap();

        // 5. suppress PII if no UV was performed in original GA

        // 6. sign
        // 7. reset timer
        // 8. increment credential counter (not applicable)

        self.assert_with_credential(None, credential)
    }

    fn credential_management(&mut self, parameters: &ctap2::credential_management::Parameters)
        -> Result<ctap2::credential_management::Response> {

        use ctap2::credential_management::Subcommand;
        use crate::credential_management as cm;

        // TODO: I see "failed pinauth" output, but then still continuation...
        self.verify_pin_auth_using_token(&parameters)?;

        let mut cred_mgmt = cm::CredentialManagement::new(self);
        let sub_parameters = &parameters.sub_command_params;
        match parameters.sub_command {

            // 0x1
            Subcommand::GetCredsMetadata =>
                cred_mgmt.get_creds_metadata(),

            // 0x2
            Subcommand::EnumerateRpsBegin =>
                cred_mgmt.first_relying_party(),

            // 0x3
            Subcommand::EnumerateRpsGetNextRp =>
                cred_mgmt.next_relying_party(),

            // 0x4
            Subcommand::EnumerateCredentialsBegin => {
                let sub_parameters = sub_parameters.as_ref()
                    .ok_or(Error::MissingParameter)?;

                cred_mgmt.first_credential(
                    sub_parameters
                        .rp_id_hash.as_ref()
                        .ok_or(Error::MissingParameter)?,
                )
            }

            // 0x5
            Subcommand::EnumerateCredentialsGetNextCredential =>
                cred_mgmt.next_credential(),

            // 0x6
            Subcommand::DeleteCredential => {
                let sub_parameters = sub_parameters.as_ref()
                    .ok_or(Error::MissingParameter)?;

                cred_mgmt.delete_credential(sub_parameters
                        .credential_id.as_ref()
                        .ok_or(Error::MissingParameter)?,
                    )
            }

            // _ => todo!("not implemented yet"),
        }
    }

    fn get_assertion(&mut self, parameters: &ctap2::get_assertion::Parameters) -> Result<ctap2::get_assertion::Response> {

        let rp_id_hash = self.hash(&parameters.rp_id.as_ref());

        // 1-4.
        let uv_performed = match self.pin_prechecks(
                &parameters.options, &parameters.pin_auth, &parameters.pin_protocol,
                &parameters.client_data_hash.as_ref(),
        ) {
            Ok(b) => b,
            Err(Error::PinRequired) => {
                // UV is optional for get_assertion
                false
            }
            Err(err) => return Err(err),
        };

        // 5. Locate eligible credentials
        //
        // Note: If allowList is passed, credential is Some(credential)
        // If no allowList is passed, credential is None and the retrieved credentials
        // are stored in state.runtime.credential_heap
        self.locate_credentials(&rp_id_hash, &parameters.allow_list, uv_performed)?;

        let credential = self.state.runtime.pop_credential_from_heap(&mut self.trussed);
        let num_credentials = match self.state.runtime.credential_heap().len() {
            0 => None,
            n => Some(n as u32 + 1),
        };
        info!("FIRST cred: {:?}",&credential);
        info!("FIRST NUM creds: {:?}",num_credentials);

        // NB: misleading, if we have "1" we return "None"
        let human_num_credentials = match num_credentials {
            Some(n) => n,
            None => 1,
        };
        info!("found {:?} applicable credentials", human_num_credentials);

        // 6. process any options present

        // UP occurs by default, but option could specify not to.
        let do_up = if parameters.options.is_some() {
            parameters.options.as_ref().unwrap().up.unwrap_or(true)
        } else {
            true
        };

        // 7. collect user presence
        let up_performed = if do_up {
            if self.up.user_present(&mut self.trussed, constants::FIDO2_UP_TIMEOUT) {
                true
            } else {
                return Err(Error::OperationDenied);
            }
        } else {
            false
        };

        let multiple_credentials = human_num_credentials > 1;
        self.state.runtime.active_get_assertion = Some(state::ActiveGetAssertionData {
            rp_id_hash: {
                let mut buf = [0u8; 32];
                buf.copy_from_slice(&rp_id_hash);
                buf
            },
            client_data_hash: {
                let mut buf = [0u8; 32];
                buf.copy_from_slice(&parameters.client_data_hash);
                buf
            },
            uv_performed,
            up_performed,
            multiple_credentials,
            extensions: parameters.extensions.clone(),
        });

        self.assert_with_credential(num_credentials, credential)
    }

    #[inline(never)]
    fn process_assertion_extensions(&mut self, 
        get_assertion_state: &state::ActiveGetAssertionData, 
        extensions: &ctap2::get_assertion::ExtensionsInput, 
        _credential: &Credential,
        credential_key_handle: ObjectHandle,
    ) -> Result<Option<ctap2::get_assertion::ExtensionsOutput>> {
        if let Some(hmac_secret) = &extensions.hmac_secret {

            // We derive credRandom as an hmac of the existing private key.
            // UV is used as input data since credRandom should depend UV
            // i.e. credRandom = HMAC(private_key, uv)
            let cred_random = syscall!(self.trussed.derive_key(
                Mechanism::HmacSha256,
                credential_key_handle,
                Some(Bytes::try_from_slice(&[get_assertion_state.uv_performed as u8]).unwrap()),
                trussed::types::StorageAttributes::new().set_persistence(Location::Volatile)
            )).key;    

            // Verify the auth tag, which uses the same process as the pinAuth
            let kek = self.state.runtime.generate_shared_secret(&mut self.trussed, &hmac_secret.key_agreement)?;
            self.verify_pin_auth(kek, &hmac_secret.salt_enc, &hmac_secret.salt_auth).map_err(|_| Error::ExtensionFirst)?;

            if hmac_secret.salt_enc.len() != 32 && hmac_secret.salt_enc.len() != 64 {
                return Err(Error::InvalidLength);
            }

            // decrypt input salt_enc to get salt1 or (salt1 || salt2)
            let salts = syscall!(
                self.trussed.decrypt(Mechanism::Aes256Cbc, kek, &hmac_secret.salt_enc, b"", b"", b"")
            ).plaintext.ok_or(Error::InvalidOption)?;

            let mut salt_output: Bytes<consts::U64> = Bytes::new();

            // output1 = hmac_sha256(credRandom, salt1)
            let output1 = syscall!(
                self.trussed.sign_hmacsha256(cred_random, &salts[0..32])
            ).signature;

            salt_output.extend_from_slice(&output1).unwrap();

            if salts.len() == 64 {
                // output2 = hmac_sha256(credRandom, salt2)
                let output2 = syscall!(
                    self.trussed.sign_hmacsha256(cred_random, &salts[32..64])
                ).signature;

                salt_output.extend_from_slice(&output2).unwrap();
            }

            syscall!(self.trussed.delete(cred_random));

            // output_enc = aes256-cbc(sharedSecret, IV=0, output1 || output2)
            let output_enc = syscall!(
                self.trussed.encrypt(Mechanism::Aes256Cbc, kek, &salt_output, b"", None)
            ).ciphertext;

            Ok(Some(ctap2::get_assertion::ExtensionsOutput {
                hmac_secret: Some(Bytes::try_from_slice(&output_enc).unwrap())
            }))

        } else {
            Ok(None)
        }
       
    }


    fn assert_with_credential(&mut self, num_credentials: Option<u32>, credential: Credential)
        -> Result<ctap2::get_assertion::Response>
    {
        let data = self.state.runtime.active_get_assertion.clone().unwrap();
        let rp_id_hash = Bytes::try_from_slice(&data.rp_id_hash).unwrap();

        let (key, is_rk) = match credential.key.clone() {
            Key::ResidentKey(key) => (key, true),
            Key::WrappedKey(bytes) => {
                let wrapping_key = self.state.persistent.key_wrapping_key(&mut self.trussed)?;
                // info!("unwrapping {:?} with wrapping key {:?}", &bytes, &wrapping_key);
                let key_result = syscall!(self.trussed.unwrap_key_chacha8poly1305(
                    wrapping_key,
                    &bytes,
                    b"",
                    // &rp_id_hash,
                    Location::Volatile,
                )).key;
                // debug!("key result: {:?}", &key_result);
                info!("key result");
                match key_result {
                    Some(key) => (key, false),
                    None => { return Err(Error::Other); }
                }
            }
        };

        // 8. process any extensions present
        let extensions_output = if let Some(extensions) = &data.extensions {
            self.process_assertion_extensions(&data, &extensions, &credential, key)?
        } else {
            None
        };

        // 9./10. sign clientDataHash || authData with "first" credential

        // info!("signing with credential {:?}", &credential);
        let kek = self.state.persistent.key_encryption_key(&mut self.trussed)?;
        let credential_id = credential.id_using_hash(&mut self.trussed, kek, &rp_id_hash)?;

        use ctap2::AuthenticatorDataFlags as Flags;

        let sig_count = self.state.persistent.timestamp(&mut self.trussed)?;

        let authenticator_data = ctap2::get_assertion::AuthenticatorData {
            rp_id_hash: rp_id_hash,

            flags: {
                let mut flags = Flags::EMPTY;
                if data.up_performed {
                    flags |= Flags::USER_PRESENCE;
                }
                if data.uv_performed {
                    flags |= Flags::USER_VERIFIED;
                }
                if extensions_output.is_some() {
                    flags |= Flags::EXTENSION_DATA;
                }
                flags
            },

            sign_count: sig_count,
            attested_credential_data: None,
            extensions: extensions_output
        };

        let serialized_auth_data = authenticator_data.serialize();

        let mut commitment = Bytes::<consts::U1024>::new();
        commitment.extend_from_slice(&serialized_auth_data).map_err(|_| Error::Other)?;
        commitment.extend_from_slice(&data.client_data_hash).map_err(|_| Error::Other)?;

        let (mechanism, serialization) = match credential.algorithm {
            -7 => (Mechanism::P256, SignatureSerialization::Asn1Der),
            -8 => (Mechanism::Ed255, SignatureSerialization::Raw),
            -9 => (Mechanism::Totp, SignatureSerialization::Raw),
            _ => { return Err(Error::Other); }
        };

        debug!("signing with {:?}, {:?}", &mechanism, &serialization);
        let signature = match mechanism {
            Mechanism::Totp => {
                let timestamp = u64::from_le_bytes(data.client_data_hash[..8].try_into().unwrap());
                info!("TOTP with timestamp {:?}", &timestamp);
                syscall!(self.trussed.sign_totp(key, timestamp)).signature.to_bytes()
            }
            _ => syscall!(self.trussed.sign(mechanism, key.clone(), &commitment, serialization)).signature
                     .to_bytes(),
        };

        if !is_rk {
            syscall!(self.trussed.delete(key));
        }

        let mut response = ctap2::get_assertion::Response {
            credential: Some(credential_id.into()),
            auth_data: Bytes::try_from_slice(&serialized_auth_data).map_err(|_| Error::Other)?,
            signature,
            user: None,
            number_of_credentials: num_credentials,
        };

        if is_rk {
            let mut user = credential.user.clone();
            // User identifiable information (name, DisplayName, icon) MUST not
            // be returned if user verification is not done by the authenticator.
            // For single account per RP case, authenticator returns "id" field.
            if !data.uv_performed || !data.multiple_credentials {
                user.icon = None;
                user.name = None;
                user.display_name = None;
            }
            response.user = Some(user);
        }

        Ok(response)
    }

    fn vendor(&mut self, op: VendorOperation) -> Result<()> {
        info!("hello VO {:?}", &op);
        match op.into() {
            0x79 => syscall!(self.trussed.debug_dump_store()),
            _ => return Err(Error::InvalidCommand),
        };

        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        // 1. >10s after bootup -> NotAllowed
        let uptime = syscall!(self.trussed.uptime()).uptime;
        if uptime.as_secs() > 10 {
            #[cfg(not(feature = "disable-reset-time-window"))]
            return Err(Error::NotAllowed);
        }
        // 2. check for user presence
        // denied -> OperationDenied
        // timeout -> UserActionTimeout
        if !self.up.user_present(&mut self.trussed, constants::U2F_UP_TIMEOUT) {
            return Err(Error::OperationDenied);
        }

        // Delete resident keys
        syscall!(self.trussed.delete_all(Location::Internal));
        syscall!(self.trussed.remove_dir_all(
            Location::Internal,
            PathBuf::from("rk"),
        ));

        // b. delete persistent state
        self.state.persistent.reset(&mut self.trussed)?;

        // c. Reset runtime state
        self.state.runtime.reset(&mut self.trussed);

        Ok(())
    }

    pub fn delete_resident_key_by_user_id(
        &mut self,
        rp_id_hash: &Bytes32,
        user_id: &Bytes<consts::U64>,
    ) -> Result<()> {

        // Prepare to iterate over all credentials associated to RP.
        let rp_path = rp_rk_dir(&rp_id_hash);
        let mut entry = syscall!(self.trussed.read_dir_first(
            Location::Internal,
            rp_path.clone(),
            None,
        )).entry;

        loop {
            info!("this may be an RK: {:?}", &entry);
            let rk_path = match entry {
                // no more RKs left
                // break breaks inner loop here
                None => break,
                Some(entry) => PathBuf::from(entry.path()),
            };

            info!("checking RK {:?} for userId ", &rk_path);
            let credential_data = syscall!(self.trussed.read_file(
                Location::Internal,
                PathBuf::from(rk_path.clone()),
            )).data;
            let credential_maybe = Credential::deserialize(&credential_data);

            if let Ok(old_credential) = credential_maybe {
                if old_credential.user.id == user_id {
                    match old_credential.key {
                        credential::Key::ResidentKey(key) => {
                            info!(":: deleting resident key");
                            syscall!(self.trussed.delete(key));
                        }
                        _ => {
                            warn!(":: WARNING: unexpected server credential in rk.");
                        }
                    }
                    syscall!(self.trussed.remove_file(
                        Location::Internal,
                        PathBuf::from(rk_path),
                    ));

                    info!("Overwriting previous rk tied to this userId.");
                    break;
                }
            } else {
                warn_now!("WARNING: Could not read RK.");
            }

            // prepare for next loop iteration
            entry = syscall!(self.trussed.read_dir_next()).entry;
        }

        Ok(())

    }

    pub fn delete_resident_key_by_path(
        &mut self,
        rk_path: &Path,
    )
        // rp_id_hash: &Bytes32,
        // credential_id_hash: &Bytes32,
    // )
        -> Result<()>
    {
        info!("deleting RK {:?}", &rk_path);
        let credential_data = syscall!(self.trussed.read_file(
            Location::Internal,
            PathBuf::from(rk_path),
        )).data;
        let credential_maybe = Credential::deserialize(&credential_data);
        // info!("deleting credential {:?}", &credential);


        if let Ok(credential) = credential_maybe {

            match credential.key {
                credential::Key::ResidentKey(key) => {
                    info!(":: deleting resident key");
                    syscall!(self.trussed.delete(key));
                }
                credential::Key::WrappedKey(_) => {}
            }
        } else {
            // If for some reason there becomes a corrupt credential,
            // we can still at least orphan the key rather then crash.
            info!("Warning!  Orpaning a key.");
        }

        info!(":: deleting RK file {:?} itself", &rk_path);
        syscall!(self.trussed.remove_file(
            Location::Internal,
            PathBuf::from(rk_path),
        ));


        Ok(())
    }

    fn hash(&mut self, data: &[u8]) -> Bytes<consts::U32> {
        let hash = syscall!(self.trussed.hash_sha256(&data)).hash;
        hash.try_to_bytes().expect("hash should fit")
    }

    fn make_credential(&mut self, parameters: &ctap2::make_credential::Parameters) -> Result<ctap2::make_credential::Response> {

        let rp_id_hash = self.hash(&parameters.rp.id.as_ref());

        // 1-4.
        if let Some(options) = parameters.options.as_ref() {
            // up option is not valid for make_credential
            if options.up.is_some() {
                return Err(Error::InvalidOption);
            }
        }
        let uv_performed = self.pin_prechecks(
            &parameters.options, &parameters.pin_auth, &parameters.pin_protocol,
            &parameters.client_data_hash.as_ref(),
        )?;

        // 5. "persist credProtect value for this credential"
        // --> seems out of place here, see 9.

        // 6. excludeList present, contains credential ID on this authenticator bound to RP?
        // --> wait for UP, error CredentialExcluded
        if let Some(exclude_list) = &parameters.exclude_list {
            for descriptor in exclude_list.iter() {
                let result = Credential::try_from(self, &rp_id_hash, descriptor);
                if let Ok(excluded_cred) = result {
                    // If UV is not performed, than CredProtectRequired credentials should not be visibile.
                    if !(excluded_cred.cred_protect == Some(CredentialProtectionPolicy::Required) && !uv_performed) {
                        info!("Excluded!");
                        if self.up.user_present(&mut self.trussed, constants::FIDO2_UP_TIMEOUT) {
                            return Err(Error::CredentialExcluded);
                        } else {
                            return Err(Error::OperationDenied);
                        }
                    }
                }
            }
        }

        // 7. check pubKeyCredParams algorithm is valid + supported COSE identifier

        let mut algorithm: Option<SupportedAlgorithm> = None;
        for param in parameters.pub_key_cred_params.iter() {
            match param.alg {
                -7 => { if algorithm.is_none() { algorithm = Some(SupportedAlgorithm::P256); }}
                -8 => { algorithm = Some(SupportedAlgorithm::Ed25519); }
                -9 => { algorithm = Some(SupportedAlgorithm::Totp); }
                _ => {}
            }
        }
        let algorithm = match algorithm {
            Some(algorithm) => {
                info!("algo: {:?}", algorithm as i32);
                algorithm
            },
            None => { return Err(Error::UnsupportedAlgorithm); }
        };
        // debug!("making credential, eddsa = {}", eddsa);


        // 8. process options; on known but unsupported error UnsupportedOption

        let mut rk_requested = false;
        // TODO: why is this unused?
        let mut _uv_requested = false;
        let _up_requested = true; // can't be toggled

        info!("MC options: {:?}", &parameters.options);
        if let Some(ref options) = &parameters.options {
            if Some(true) == options.rk {
                rk_requested = true;
            }
            if Some(true) == options.uv {
                _uv_requested = true;
            }
        }

        // 9. process extensions
        let mut hmac_secret_requested = None;
        // let mut cred_protect_requested = CredentialProtectionPolicy::Optional;
        let mut cred_protect_requested = None;
        if let Some(extensions) = &parameters.extensions {
            
            hmac_secret_requested = extensions.hmac_secret;

            if let Some(policy) = &extensions.cred_protect {
                cred_protect_requested = Some(CredentialProtectionPolicy::try_from(*policy)?);
            }
        }

        // debug!("hmac-secret = {:?}, credProtect = {:?}", hmac_secret_requested, cred_protect_requested);

        // 10. get UP, if denied error OperationDenied
        if !self.up.user_present(&mut self.trussed, constants::FIDO2_UP_TIMEOUT) {
            return Err(Error::OperationDenied);
        }

        // 11. generate credential keypair
        let location = match rk_requested {
            true => Location::Internal,
            false => Location::Volatile,
        };

        let private_key: ObjectHandle;
        let public_key: ObjectHandle;
        let cose_public_key;
        match algorithm {
            SupportedAlgorithm::P256 => {
                private_key = syscall!(self.trussed.generate_p256_private_key(location)).key;
                public_key = syscall!(self.trussed.derive_p256_public_key(private_key, Location::Volatile)).key;
                cose_public_key = syscall!(self.trussed.serialize_key(
                    Mechanism::P256, public_key.clone(), KeySerialization::Cose
                )).serialized_key;
                let _success = syscall!(self.trussed.delete(public_key)).success;
                info!("deleted public P256 key: {}", _success);
            }
            SupportedAlgorithm::Ed25519 => {
                private_key = syscall!(self.trussed.generate_ed255_private_key(location)).key;
                public_key = syscall!(self.trussed.derive_ed255_public_key(private_key, Location::Volatile)).key;
                cose_public_key = syscall!(self.trussed.serialize_key(
                    Mechanism::Ed255, public_key.clone(), KeySerialization::Cose
                )).serialized_key;
                let _success = syscall!(self.trussed.delete(public_key)).success;
                info!("deleted public Ed25519 key: {}", _success);
            }
            SupportedAlgorithm::Totp => {
                if parameters.client_data_hash.len() != 32 {
                    return Err(Error::InvalidParameter);
                }
                // b'TOTP---W\x0e\xf1\xe0\xd7\x83\xfe\t\xd1\xc1U\xbf\x08T_\x07v\xb2\xc6--TOTP'
                let totp_secret: [u8; 20] = parameters.client_data_hash[6..26].try_into().unwrap();
                private_key = syscall!(self.trussed.unsafe_inject_shared_key(
                    &totp_secret, Location::Internal)).key;
                // info!("totes injected");
                let fake_cose_pk = ctap_types::cose::TotpPublicKey {};
                let fake_serialized_cose_pk = trussed::cbor_serialize_bytes(&fake_cose_pk)
                    .map_err(|_| Error::NotAllowed)?;
                cose_public_key = fake_serialized_cose_pk; // Bytes::try_from_slice(&[0u8; 20]).unwrap();
            }
        }

        // 12. if `rk` is set, store or overwrite key pair, if full error KeyStoreFull

        // 12.a generate credential
        let key_parameter = match rk_requested {
            true => Key::ResidentKey(private_key),
            false => {
                // WrappedKey version
                let wrapping_key = self.state.persistent.key_wrapping_key(&mut self.trussed)?;
                debug!("wrapping private key");
                let wrapped_key = syscall!(self.trussed.wrap_key_chacha8poly1305(
                    wrapping_key,
                    private_key,
                    &rp_id_hash,
                )).wrapped_key;
                // debug!("wrapped_key = {:?}", &wrapped_key);

                // 32B key, 12B nonce, 16B tag + some info on algorithm (P256/Ed25519)
                // Turns out it's size 92 (enum serialization not optimized yet...)
                // let mut wrapped_key = Bytes::<consts::U60>::new();
                // wrapped_key.extend_from_slice(&wrapped_key_msg).unwrap();
                let ret = Key::WrappedKey(wrapped_key.try_to_bytes().map_err(|_| Error::Other)?);
                ret
                // debug!("len wrapped key = {}", wrapped_key.len());
                // Key::WrappedKey(wrapped_key.try_to_bytes().unwrap())

            }
        };

        // injecting this is a bit mehhh..
        let nonce = syscall!(self.trussed.random_bytes(12)).bytes.as_slice().try_into().unwrap();
        info!("nonce = {:?}", &nonce);

        let credential = Credential::new(
            credential::CtapVersion::Fido21Pre,
            &parameters.rp,
            &parameters.user,
            algorithm as i32,
            key_parameter,
            self.state.persistent.timestamp(&mut self.trussed)?,
            hmac_secret_requested.clone(),
            cred_protect_requested,
            nonce,
        );

        // info!("made credential {:?}", &credential);

        // 12.b generate credential ID { = AEAD(Serialize(Credential)) }
        let kek = self.state.persistent.key_encryption_key(&mut self.trussed)?;
        let credential_id = credential.id_using_hash(&mut self.trussed, kek, &rp_id_hash)?;

        // store it.
        // TODO: overwrite, error handling with KeyStoreFull

        let serialized_credential = credential.serialize()?;


        if rk_requested {
            // first delete any other RK cred with same RP + UserId if there is one.
            self.delete_resident_key_by_user_id(&rp_id_hash, &credential.user.id).ok();

            let credential_id_hash = self.hash(&credential_id.0.as_ref());
            try_syscall!(self.trussed.write_file(
                Location::Internal,
                rk_path(&rp_id_hash, &credential_id_hash),
                serialized_credential.clone(),
                // user attribute for later easy lookup
                // Some(rp_id_hash.clone()),
                None,
            )).map_err(|_| Error::KeyStoreFull)?;
        }
        // 13. generate and return attestation statement using clientDataHash

        // 13.a AuthenticatorData and its serialization
        use ctap2::AuthenticatorDataFlags as Flags;
        info!("MC created cred id");

        let (attestation_maybe, aaguid)= self.state.identity.attestation(&mut self.trussed);

        let authenticator_data = ctap2::make_credential::AuthenticatorData {
            rp_id_hash: rp_id_hash.try_to_bytes().map_err(|_| Error::Other)?,

            flags: {
                let mut flags = Flags::USER_PRESENCE;
                if uv_performed {
                    flags |= Flags::USER_VERIFIED;
                }
                if true {
                    flags |= Flags::ATTESTED_CREDENTIAL_DATA;
                }
                if hmac_secret_requested.is_some() || cred_protect_requested.is_some() {
                    flags |= Flags::EXTENSION_DATA;
                }
                flags
            },

            sign_count: self.state.persistent.timestamp(&mut self.trussed)?,

            attested_credential_data: {
                // debug!("acd in, cid len {}, pk len {}", credential_id.0.len(), cose_public_key.len());
                let attested_credential_data = ctap2::make_credential::AttestedCredentialData {
                    aaguid: Bytes::try_from_slice(&aaguid).unwrap(),
                    credential_id: credential_id.0.try_to_bytes().unwrap(),
                    credential_public_key: cose_public_key.try_to_bytes().unwrap(),
                };
                // debug!("cose PK = {:?}", &attested_credential_data.credential_public_key);
                Some(attested_credential_data)
            },

            extensions: {
                if hmac_secret_requested.is_some() || cred_protect_requested.is_some() {
                    Some(ctap2::make_credential::Extensions {
                        cred_protect: parameters.extensions.as_ref().unwrap().cred_protect.clone(),
                        hmac_secret: parameters.extensions.as_ref().unwrap().hmac_secret.clone(),
                    })

                } else {
                    None
                }
            },
        };
        // debug!("authData = {:?}", &authenticator_data);

        let serialized_auth_data = authenticator_data.serialize();

        // 13.b The Signature

        // can we write Sum<M, N> somehow?
        // debug!("seeking commitment, {} + {}", serialized_auth_data.len(), parameters.client_data_hash.len());
        let mut commitment = Bytes::<consts::U1024>::new();
        commitment.extend_from_slice(&serialized_auth_data).map_err(|_| Error::Other)?;
        // debug!("serialized_auth_data ={:?}", &serialized_auth_data);
        commitment.extend_from_slice(&parameters.client_data_hash).map_err(|_| Error::Other)?;
        // debug!("client_data_hash = {:?}", &parameters.client_data_hash);
        // debug!("commitment = {:?}", &commitment);

        // NB: the other/normal one is called "basic" or "batch" attestation,
        // because it attests the authenticator is part of a batch: the model
        // specified by AAGUID.
        // "self signed" is also called "surrogate basic".
        //
        // we should also directly support "none" format, it's a bit weird
        // how browsers firefox this

        let (signature, attestation_algorithm) = {
            if attestation_maybe.is_none() {
                match algorithm {
                    SupportedAlgorithm::Ed25519 => {
                        let signature = syscall!(self.trussed.sign_ed255(private_key, &commitment)).signature;
                        (signature.try_to_bytes().map_err(|_| Error::Other)?, -8)
                    }

                    SupportedAlgorithm::P256 => {
                        // DO NOT prehash here, `trussed` does that
                        let der_signature = syscall!(self.trussed.sign_p256(private_key, &commitment, SignatureSerialization::Asn1Der)).signature;
                        (der_signature.try_to_bytes().map_err(|_| Error::Other)?, -7)
                    }
                    SupportedAlgorithm::Totp => {
                        // maybe we can fake it here too, but seems kinda weird
                        // return Err(Error::UnsupportedAlgorithm);
                        // micro-ecc is borked. let's self-sign anyway
                        let hash = syscall!(self.trussed.hash_sha256(&commitment.as_ref())).hash;
                        let tmp_key = syscall!(self.trussed
                            .generate_p256_private_key(Location::Volatile))
                            .key;

                        let signature = syscall!(self.trussed.sign_p256(
                            tmp_key,
                            &hash,
                            SignatureSerialization::Asn1Der,
                        )).signature;
                        (signature.try_to_bytes().map_err(|_| Error::Other)?, -7)
                    }
                }
            } else {

                let signature = syscall!(self.trussed.sign_p256(
                    attestation_maybe.as_ref().unwrap().0,
                    &commitment,
                    SignatureSerialization::Asn1Der,
                )).signature;
                (signature.try_to_bytes().map_err(|_| Error::Other)?, -7)
            }
        };
        // debug!("SIG = {:?}", &signature);

        if !rk_requested {
            let _success = syscall!(self.trussed.delete(private_key)).success;
            info!("deleted private credential key: {}", _success);
        }

        let packed_attn_stmt = ctap2::make_credential::PackedAttestationStatement {
            alg: attestation_algorithm,
            sig: signature,
            x5c: match attestation_maybe.is_some() {
                false => None,
                true => {
                    // See: https://www.w3.org/TR/webauthn-2/#sctn-packed-attestation-cert-requirements
                    let cert = attestation_maybe.as_ref().unwrap().1.clone();
                    let mut x5c = Vec::new();
                    x5c.push(cert).ok();
                    Some(x5c)
                }
            },
        };

        let fmt = String::<consts::U32>::from("packed");
        let att_stmt = ctap2::make_credential::AttestationStatement::Packed(packed_attn_stmt);

        let attestation_object = ctap2::make_credential::Response {
            fmt,
            auth_data: serialized_auth_data,
            att_stmt,
        };

        Ok(attestation_object)
    }

    // fn credential_id(credential: &Credential) -> CredentialId {
    // }

    // fn get_assertion(&mut self, ...)
    //     // let unwrapped_key = syscall!(self.trussed.unwrap_key_chacha8poly1305(
    //     //     &wrapping_key,
    //     //     &wrapped_key,
    //     //     b"",
    //     //     Location::Volatile,
    //     // )).key;
        // // test public key ser/de
        // let ser_pk = syscall!(self.trussed.serialize_key(
        //     Mechanism::P256, public_key.clone(), KeySerialization::Raw
        // )).serialized_key;
        // debug!("ser pk = {:?}", &ser_pk);

        // let cose_ser_pk = syscall!(self.trussed.serialize_key(
        //     Mechanism::P256, public_key.clone(), KeySerialization::Cose
        // )).serialized_key;
        // debug!("COSE ser pk = {:?}", &cose_ser_pk);

        // let deser_pk = syscall!(self.trussed.deserialize_key(
        //     Mechanism::P256, ser_pk.clone(), KeySerialization::Raw,
        //     StorageAttributes::new().set_persistence(Location::Volatile)
        // )).key;
        // debug!("deser pk = {:?}", &deser_pk);

        // let cose_deser_pk = syscall!(self.trussed.deserialize_key(
        //     Mechanism::P256, cose_ser_pk.clone(), KeySerialization::Cose,
        //     StorageAttributes::new().set_persistence(Location::Volatile)
        // )).key;
        // debug!("COSE deser pk = {:?}", &cose_deser_pk);
        // debug!("raw ser of COSE deser pk = {:?}",
        //           syscall!(self.trussed.serialize_key(Mechanism::P256, cose_deser_pk.clone(), KeySerialization::Raw)).
        //           serialized_key);

        // debug!("priv {:?}", &private_key);
        // debug!("pub {:?}", &public_key);

        // let _loaded_credential = syscall!(self.trussed.load_blob(
        //     prefix.clone(),
        //     blob_id,
        //     Location::Volatile,
        // )).data;
        // // debug!("loaded credential = {:?}", &loaded_credential);

        // debug!("credential = {:?}", &Credential::deserialize(&serialized_credential)?);

    //     // debug!("unwrapped_key = {:?}", &unwrapped_key);

    fn get_info(&mut self) -> ctap2::get_info::Response {

        use core::str::FromStr;
        let mut versions = Vec::<String<consts::U12>, consts::U3>::new();
        versions.push(String::from_str("U2F_V2").unwrap()).unwrap();
        versions.push(String::from_str("FIDO_2_0").unwrap()).unwrap();
        // #[cfg(feature = "enable-fido-pre")]
        // versions.push(String::from_str("FIDO_2_1_PRE").unwrap()).unwrap();

        let mut extensions = Vec::<String<consts::U11>, consts::U4>::new();
        // extensions.push(String::from_str("credProtect").unwrap()).unwrap();
        extensions.push(String::from_str("hmac-secret").unwrap()).unwrap();

        let mut pin_protocols = Vec::<u8, consts::U1>::new();
        pin_protocols.push(1).unwrap();

        let mut options = ctap2::get_info::CtapOptions::default();
        options.rk = true;
        options.up = true;
        options.uv = None; // "uv" here refers to "in itself", e.g. biometric
        // options.plat = false;
        options.cred_mgmt = Some(true);
        // options.client_pin = None; // not capable of PIN
        options.client_pin = match self.state.persistent.pin_is_set() {
            true => Some(true),
            false => Some(false),
        };

        let (_, aaguid)= self.state.identity.attestation(&mut self.trussed);

        ctap2::get_info::Response {
            versions,
            extensions: Some(extensions),
            aaguid: Bytes::try_from_slice(&aaguid).unwrap(),
            options: Some(options),
            max_msg_size: Some(ctap_types::sizes::MESSAGE_SIZE),
            pin_protocols: Some(pin_protocols),
            max_creds_in_list: Some(ctap_types::sizes::MAX_CREDENTIAL_COUNT_IN_LIST_VALUE),
            max_cred_id_length: Some(ctap_types::sizes::MAX_CREDENTIAL_ID_LENGTH_VALUE),
            ..ctap2::get_info::Response::default()
        }
    }

//     fn get_or_create_counter_handle(trussed_client: &mut TrussedClient) -> Result<ObjectHandle> {

//         // there should be either 0 or 1 counters with this name. if not, it's a logic error.
//         let attributes = Attributes {
//             kind: Counter,
//             label: Self::GLOBAL_COUNTER_NAME.into(),
//         };

//         // let reply = syscall!(FindObjects, attributes)?;

//         let reply = block!(
//             request::FindObjects { attributes }
//                 .submit(&mut trussed_client)
//                 // no pending requests
//                 .map_err(drop)
//                 .unwrap()
//         )?;

//         // how should this API look like.
//         match reply.num_objects() {
//             // the happy case
//             1 => Ok(reply.object_handles[0]),

//             // first run - create counter
//             0 => {
//                 let reply = block!(
//                     request::FindObjects { attributes }
//                         .submit(&mut trussed_client)
//                         // no pending requests
//                         .map_err(drop)
//                         .unwrap()
//                 )?;
//                 Ok(reply::ReadCounter::from(reply).object_handle)
//             }

//             // should not occur
//             _ => Err(Error::TooManyCounters),
//         }
//     }

//     fn get_or_create_counter_handle(trussed_client: &mut TrussedClient) -> Result<ObjectHandle> {
//         todo!("not implemented yet, follow counter code");
//     }

// }

// impl authenticator::Api for Authenticator
// {
//     fn get_info(&mut self) -> AuthenticatorInfo {
//         todo!();
//     }

//     fn reset(&mut self) -> Result<()> {
//         todo!();
//     }


//     fn get_assertions(&mut self, params: &GetAssertionParameters) -> Result<AssertionResponses> {
//         todo!();
//     }

//     fn make_credential(&mut self, params: &MakeCredentialParameters) -> Result<AttestationObject> {
//         todo!();
//     }

}

#[cfg(test)]
mod test {
}
