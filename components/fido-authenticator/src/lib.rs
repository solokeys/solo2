 #![cfg_attr(not(test), no_std)]

use core::convert::{TryFrom, TryInto};

#[cfg(feature = "semihosting")]
#[allow(unused_imports)]
use cortex_m_semihosting::hprintln;

use funnel::info;

use crypto_service::{
    Client as CryptoClient,
    pipe::Syscall as CryptoSyscall,
    types::{
        KeySerialization,
        Mechanism,
        MediumData,
        Message,
        ObjectHandle,
        SignatureSerialization,
        StorageLocation,
    },
};

use ctap_types::{
    Bytes, Bytes32, consts, String, Vec,
    // cose::EcdhEsHkdf256PublicKey as CoseEcdhEsHkdf256PublicKey,
    // cose::PublicKey as CosePublicKey,
    ctaphid::VendorOperation,
    rpc::AuthenticatorEndpoint,
    // authenticator::ctap1,
    authenticator::{ctap2, Error, Request, Response},
};

use littlefs2::path::{Path, PathBuf};

pub mod credential_management;
pub mod state;

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

macro_rules! block {
    ($future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $future_result;
        loop {
            match future_result.poll() {
                core::task::Poll::Ready(result) => { break result; },
                core::task::Poll::Pending => {},
            }
        }
    }}
}

#[macro_use]
macro_rules! syscall {
    ($pre_future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $pre_future_result.expect("no client error");
        loop {
            match future_result.poll() {
                // core::task::Poll::Ready(result) => { break result.expect("no errors"); },
                core::task::Poll::Ready(result) => { break result.unwrap(); },
                core::task::Poll::Pending => {},
            }
        }
    }}
}

pub mod credential;
pub use credential::*;

#[cfg(not(feature = "debug-logs"))]
#[macro_use(info)]
extern crate funnel;

#[allow(unused_imports)]
#[cfg(feature = "debug-logs")]
#[macro_use(debug,info)]
extern crate funnel;

#[cfg(not(feature = "debug-logs"))]
#[macro_use]
macro_rules! debug { ($($tt:tt)*) => {{ core::result::Result::<(), core::convert::Infallible>::Ok(()) }} }

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
enum SupportedAlgorithm {
    P256 = -7,
    Ed25519 = -8,
}

/// Idea is to maybe send a request over a queue,
/// and return upon button press.
/// TODO: Do we need a timeout?
pub trait UserPresence {
    fn user_present(&mut self) -> bool;
}

pub struct SilentAuthenticator {}

impl UserPresence for SilentAuthenticator {
    fn user_present(&mut self) -> bool {
        true
    }
}

fn cbor_serialize_message<T: serde::Serialize>(object: &T) -> core::result::Result<Message, ctap_types::serde::Error> {
    let mut message = Message::new();
    ctap_types::serde::cbor_serialize_bytes(object, &mut message)?;
    Ok(message)
}

pub struct Authenticator<'a, S, UP>
where
    S: CryptoSyscall,
    UP: UserPresence,
{
    crypto: CryptoClient<'a, S>,
    rpc: AuthenticatorEndpoint<'a>,
    state: state::State,
    up: UP,
}

impl<'a, S: CryptoSyscall, UP: UserPresence> Authenticator<'a, S, UP> {

    pub fn new(crypto: CryptoClient<'a, S>, rpc: AuthenticatorEndpoint<'a>, up: UP) -> Self {

        let crypto = crypto;
        let state = state::State::new();
        let authenticator = Authenticator { crypto, rpc, state, up };

        authenticator
    }

    fn respond(&mut self, response: Result<Response>) {
        self.rpc.send.enqueue(response).expect("internal error");
    }

    pub fn poll(&mut self) {
        match self.rpc.recv.dequeue() {
            None => {},
            Some(request) => {
                // debug!("request: {:?}", &request).ok();

                match request {
                    Request::Ctap2(request) => {
                        match request {

                            // 0x4
                            ctap2::Request::GetInfo => {
                                debug!("GI").ok();
                                let response = self.get_info();
                                self.rpc.send.enqueue(
                                    Ok(Response::Ctap2(ctap2::Response::GetInfo(response))))
                                    .expect("internal error");
                            }

                            // 0x2
                            ctap2::Request::MakeCredential(parameters) => {
                                // debug!("MC: {:?}", &parameters).ok();
                                let response = self.make_credential(&parameters);
                                self.rpc.send.enqueue(
                                    match response {
                                        Ok(response) => Ok(Response::Ctap2(ctap2::Response::MakeCredential(response))),
                                        Err(error) => Err(error)
                                    })
                                    .expect("internal error");
                                debug!("enqueued MC response").ok();
                            }

                            // 0x1
                            ctap2::Request::GetAssertion(parameters) => {
                                // debug!("GA: {:?}", &parameters).ok();
                                let response = self.get_assertion(&parameters);
                                self.rpc.send.enqueue(
                                    match response {
                                        Ok(response) => Ok(Response::Ctap2(ctap2::Response::GetAssertion(response))),
                                        Err(error) => Err(error)
                                    })
                                    .expect("internal error");
                                debug!("enqueued GA response").ok();
                            }

                            // 0x8
                            ctap2::Request::GetNextAssertion => {
                                // debug!("GNA: {:?}", &parameters).ok();
                                let response = self.get_next_assertion();
                                self.rpc.send.enqueue(
                                    match response {
                                        Ok(response) => Ok(Response::Ctap2(ctap2::Response::GetNextAssertion(response))),
                                        Err(error) => Err(error)
                                    })
                                    .expect("internal error");
                                debug!("enqueued GA response").ok();
                            }

                            // 0x7
                            ctap2::Request::Reset => {
                                // debug!("RESET: {:?}", &parameters).ok();
                                let response = self.reset();
                                self.rpc.send.enqueue(
                                    match response {
                                        Ok(()) => Ok(Response::Ctap2(ctap2::Response::Reset)),
                                        Err(error) => Err(error)
                                    })
                                    .expect("internal error");
                                debug!("enqueued GA response").ok();
                            }


                            // 0x6
                            ctap2::Request::ClientPin(parameters) => {
                                let response = self.client_pin(&parameters);
                                // #[cfg(feature = "semihosting")]
                                // hprintln!("{:?}", &response).ok();
                                self.rpc.send.enqueue(
                                    match response {
                                        Ok(response) => Ok(Response::Ctap2(ctap2::Response::ClientPin(response))),
                                        Err(error) => Err(error)
                                    })
                                    .expect("internal error");
                                debug!("enqueued CP response").ok();
                            }

                            // 0xA
                            ctap2::Request::CredentialManagement(parameters) => {
                                // debug!("CM: {:?}", &parameters).ok();
                                let response = self.credential_management(&parameters);
                                self.rpc.send.enqueue(
                                    match response {
                                        Ok(response) => Ok(Response::Ctap2(ctap2::Response::CredentialManagement(response))),
                                        Err(error) => Err(error)
                                    })
                                    .expect("internal error");
                                debug!("enqueued GA response").ok();
                            }


                            ctap2::Request::Vendor(op) => {
                                let response = self.vendor(op);
                                self.rpc.send.enqueue(
                                    match response {
                                        Ok(()) => Ok(Response::Ctap2(ctap2::Response::Vendor)),
                                        Err(error) => Err(error)
                                    })
                                    .expect("internal error");
                            }

                            // _ => {
                            //     // debug!("not implemented: {:?}", &request).ok();
                            //     debug!("request not implemented").ok();
                            //     self.rpc.send.enqueue(Err(Error::InvalidCommand)).expect("internal error");
                            // }
                        }
                    }
                    Request::Ctap1(_request) => {
                        debug!("ctap1 not implemented: {:?}", &request).ok();
                        // self.rpc.send.enqueue(Err(Error::InvalidCommand)).expect("internal error");
                        self.respond(Err(Error::InvalidCommand));
                    }
                }
            }
        }
    }

    fn client_pin(&mut self, parameters: &ctap2::client_pin::Parameters) -> Result<ctap2::client_pin::Response> {
        use ctap2::client_pin::PinV1Subcommand as Subcommand;
        debug!("processing CP").ok();
        // #[cfg(feature = "semihosting")]
        // hprintln!("{:?}", parameters).ok();

        if parameters.pin_protocol != 1{
            return Err(Error::InvalidParameter);
        }

        Ok(match parameters.sub_command {

            Subcommand::GetRetries => {
                debug!("processing CP.GR").ok();

                ctap2::client_pin::Response {
                    key_agreement: None,
                    pin_token: None,
                    retries: Some(self.state.persistent.retries()),
                }
            }

            Subcommand::GetKeyAgreement => {
                debug!("processing CP.GKA").ok();

                let private_key = self.state.runtime.key_agreement_key(&mut self.crypto);
                let public_key = syscall!(self.crypto.derive_p256_public_key(&private_key, StorageLocation::Volatile)).key;
                let serialized_cose_key = syscall!(self.crypto.serialize_key(
                    Mechanism::P256, public_key.clone(), KeySerialization::EcdhEsHkdf256)).serialized_key;
                let cose_key = crypto_service::cbor_deserialize(&serialized_cose_key).unwrap();

                // TODO: delete public key
                syscall!(self.crypto.delete(public_key));

                ctap2::client_pin::Response {
                    key_agreement: cose_key,
                    pin_token: None,
                    retries: None,
                }
            }

            Subcommand::SetPin => {
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
                    return Err(Error::PinAuthInvalid);
                }

                // 3. generate shared secret
                let shared_secret = self.state.runtime.generate_shared_secret(&mut self.crypto, platform_kek)?;

                // TODO: there are moar early returns!!
                // - implement Drop?
                // - do garbage collection outside of this?

                // 4. verify pinAuth
                self.verify_pin_auth(&shared_secret, new_pin_enc, pin_auth)?;

                // 5. decrypt and verify new PIN
                let new_pin = self.decrypt_pin_check_length(&shared_secret, new_pin_enc)?;

                syscall!(self.crypto.delete(shared_secret));

                // 6. store LEFT(SHA-256(newPin), 16), set retries to 8
                self.hash_store_pin(&new_pin)?;
                self.state.reset_retries(&mut self.crypto).map_err(|_| Error::Other)?;

                ctap2::client_pin::Response {
                    key_agreement: None,
                    pin_token: None,
                    retries: None,
                }
            }

            Subcommand::ChangePin => {

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
                if self.state.persistent.retries() == 0 {
                    return Err(Error::PinBlocked);
                }

                // 3. generate shared secret
                let shared_secret = self.state.runtime.generate_shared_secret(&mut self.crypto, platform_kek)?;

                // 4. verify pinAuth
                let mut data = MediumData::new();
                data.extend_from_slice(new_pin_enc).map_err(|_| Error::InvalidParameter)?;
                data.extend_from_slice(pin_hash_enc).map_err(|_| Error::InvalidParameter)?;
                self.verify_pin_auth(&shared_secret, &data, pin_auth)?;

                // 5. decrement retries
                self.state.decrement_retries(&mut self.crypto)?;

                // 6. decrypt pinHashEnc, compare with stored
                match self.decrypt_pin_hash_and_maybe_escalate(&shared_secret, &pin_hash_enc) {
                    Err(e) => {
                        syscall!(self.crypto.delete(shared_secret));
                        return Err(e);
                    }
                    Ok(_) => {}
                }

                // 7. reset retries
                self.state.reset_retries(&mut self.crypto)?;

                // 8. decrypt and verify new PIN
                let new_pin = self.decrypt_pin_check_length(&shared_secret, new_pin_enc)?;

                syscall!(self.crypto.delete(shared_secret));

                // 9. store hashed PIN
                self.hash_store_pin(&new_pin)?;

                ctap2::client_pin::Response {
                    key_agreement: None,
                    pin_token: None,
                    retries: None,
                }
            }

            Subcommand::GetPinToken => {
                debug!("processing CP.GKA").ok();

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
                if self.state.persistent.retries() == 0 {
                    return Err(Error::PinBlocked);
                }

                // 3. generate shared secret
                let shared_secret = self.state.runtime.generate_shared_secret(&mut self.crypto, platform_kek)?;

                // 4. decrement retires
                self.state.decrement_retries(&mut self.crypto)?;

                // 5. decrypt and verify pinHashEnc
                match self.decrypt_pin_hash_and_maybe_escalate(&shared_secret, &pin_hash_enc) {
                    Err(e) => {
                        syscall!(self.crypto.delete(shared_secret));
                        return Err(e);
                    }
                    Ok(_) => {}
                }
                // hprintln!("exists? {}", syscall!(self.crypto.exists(shared_secret)).exists).ok();

                // 6. reset retries
                self.state.reset_retries(&mut self.crypto)?;

                // 7. return encrypted pinToken
                let pin_token = self.state.runtime.pin_token(&mut self.crypto);
                debug!("wrapping pin token").ok();
                // hprintln!("exists? {}", syscall!(self.crypto.exists(shared_secret)).exists).ok();
                let pin_token_enc = syscall!(self.crypto.wrap_key_aes256cbc(&shared_secret, &pin_token)).wrapped_key;

                syscall!(self.crypto.delete(shared_secret));

                // ble...
                if pin_token_enc.len() != 16 {
                    return Err(Error::Other);
                }
                let pin_token_enc_32 = Bytes::<consts::U32>::try_from_slice(&pin_token_enc).unwrap();

                ctap2::client_pin::Response {
                    key_agreement: None,
                    pin_token: Some(pin_token_enc_32),
                    retries: None,
                }
            }

        })
    }

    fn decrypt_pin_hash_and_maybe_escalate(&mut self, shared_secret: &ObjectHandle, pin_hash_enc: &Bytes<consts::U64>)
        -> Result<()>
    {
        let pin_hash = syscall!(self.crypto.decrypt_aes256cbc(
            &shared_secret, pin_hash_enc)).plaintext.ok_or(Error::Other)?;

        let stored_pin_hash = match self.state.persistent.pin_hash() {
            Some(hash) => hash,
            None => { return Err(Error::InvalidCommand); }
        };

        if &pin_hash != &stored_pin_hash {
            // I) generate new KEK
            self.state.runtime.rotate_key_agreement_key(&mut self.crypto);
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
        let pin_hash_32 = syscall!(self.crypto.hash_sha256(&pin)).hash;
        let pin_hash: [u8; 16] = pin_hash_32[..16].try_into().unwrap();
        self.state.persistent.set_pin_hash(&mut self.crypto, pin_hash).unwrap();

        Ok(())
    }

    fn decrypt_pin_check_length(&mut self, shared_secret: &ObjectHandle, pin_enc: &[u8]) -> Result<Message> {
        // pin is expected to be filled with null bytes to length at least 64
        if pin_enc.len() < 64 {
            // correct error?
            return Err(Error::PinPolicyViolation);
        }

        let mut pin = syscall!(self.crypto.decrypt_aes256cbc(
            &shared_secret, &pin_enc)).plaintext.ok_or(Error::Other)?;

        // // temp
        // let pin_length = pin.iter().position(|&b| b == b'\0').unwrap_or(pin.len());
        // #[cfg(feature = "semihosting")]
        // hprintln!("pin.len() = {}, pin_length = {}, = {:?}",
        //           pin.len(), pin_length, &pin).ok();
        // chop off null bytes
        let pin_length = pin.iter().position(|&b| b == b'\0').unwrap_or(pin.len());
        if pin_length < 4 {
            return Err(Error::PinPolicyViolation);
        }

        pin.resize_default(pin_length).unwrap();

        Ok(pin)
    }


    // fn verify_pin(&mut self, pin_auth: &Bytes<consts::U16>, client_data_hash: &Bytes<consts::U32>) -> bool {
    fn verify_pin(&mut self, pin_auth: &[u8; 16], data: &[u8]) -> Result<()> {
        let key = self.state.runtime.pin_token(&mut self.crypto);
        let tag = syscall!(self.crypto.sign_hmacsha256(&key, data)).signature;
        if pin_auth == &tag[..16] {
            Ok(())
        } else {
            Err(Error::PinAuthInvalid)
        }
    }

    fn verify_pin_auth(&mut self, shared_secret: &ObjectHandle, data: &[u8], pin_auth: &Bytes<consts::U16>)
        -> Result<()>
    {
        let expected_pin_auth = syscall!(self.crypto.sign_hmacsha256(shared_secret, data)).signature;

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

        // hprintln!("CM params: {:?}", parameters).ok();
        match parameters.sub_command as u8 {
            // are we Haskell yet lol
            sub_command @ 1 |
            sub_command @ 2 |
            sub_command @ 4 |
            sub_command @ 6 => {

                // check pinProtocol
                let pin_protocol = parameters
                    // .sub_command_params.as_ref().ok_or(Error::MissingParameter)?
                    .pin_protocol.ok_or(Error::MissingParameter)?;
                if pin_protocol != 1 {
                    return Err(Error::InvalidParameter);
                }

                // check pinAuth
                let pin_token = self.state.runtime.pin_token(&mut self.crypto);
                let mut data: Bytes<consts::U256> = Bytes::try_from_slice(&[sub_command]).unwrap();
                let len = 1 + match sub_command {
                    4 | 6 => {
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

                // hprintln!("input to hmacsha256: {:?}", &data[..len]).ok();
                let expected_pin_auth = syscall!(self.crypto.sign_hmacsha256(
                    &pin_token,
                    &data[..len],
                )).signature;

                let pin_auth = parameters
                    .pin_auth.as_ref().ok_or(Error::MissingParameter)?;

                if &expected_pin_auth[..16] == &pin_auth[..] {
                    hprintln!("passed pinauth").ok();
                    Ok(())
                } else {
                    hprintln!("failed pinauth, expected {:?} got {:?}",
                              &expected_pin_auth[..16],
                              &pin_auth[..],
                    ).ok();
                    self.state.decrement_retries(&mut self.crypto)?;
                    self.state.pin_blocked()
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
                if !self.up.user_present() {
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
                        pin_auth.as_ref().try_into().unwrap(),
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
    fn locate_credentials(
        &mut self, rp_id_hash: &Bytes32,
        allow_list: &Option<ctap2::get_assertion::AllowList>,
        uv_performed: bool,
    )
        -> Result<Option<Credential>>
    {
        // validate allowList
        let allowed_credentials = if let Some(allow_list) = allow_list.as_ref() {
            let valid_allowed_credentials: CredentialList = allow_list.into_iter()
                // discard not properly serialized encrypted credentials
                .filter_map(|credential_descriptor| {
                    Credential::try_from(
                        self, rp_id_hash, credential_descriptor)
                        .ok()
                } )
                .collect();
            if valid_allowed_credentials.len() < allow_list.len() {
                debug!("invalid credential").ok();
                return Err(Error::InvalidCredential);
            }
            debug!("allowedList passed").ok();
            valid_allowed_credentials
        } else {
            debug!("no allowedList passed").ok();
            CredentialList::new()
        };

        let allowed_credentials_passed = allowed_credentials.len() > 0;

        let allowed_credential = if allowed_credentials_passed {
            // "If an allowList is present and is non-empty,
            // locate all denoted credentials present on this authenticator
            // and bound to the specified rpId."
            let mut applicable_credentials: CredentialList = allowed_credentials
                .into_iter()
                .filter(|credential| match credential.key.clone() {
                    // TODO: should check if wrapped key is valid AEAD
                    // On the other hand, we already decrypted a valid AEAD
                    Key::WrappedKey(_) => true,
                    Key::ResidentKey(key) => {
                        debug!("checking if ResidentKey {:?} exists", &key).ok();
                        match credential.algorithm {
                            -7 => syscall!(self.crypto.exists(Mechanism::P256, key)).exists,
                            -8 => syscall!(self.crypto.exists(Mechanism::Ed25519, key)).exists,
                            _ => false,
                        }
                    }
                })
                .filter(|credential| {
                    use credential::CredentialProtectionPolicy as Policy;
                    debug!("CredentialProtectionPolicy {:?}", &credential.cred_protect).ok();
                    match credential.cred_protect {
                        Policy::Optional => true,
                        Policy::OptionalWithCredentialIdList => allowed_credentials_passed || uv_performed,
                        Policy::Required => uv_performed,

                    }
                })
                .collect();
            while applicable_credentials.len() > 1 {
                applicable_credentials.pop().unwrap();
            }
            Some(applicable_credentials.pop().unwrap())
        } else {
            // If an allowList is not present,
            // locate all credentials that are present on this authenticator
            // and bound to the specified rpId; sorted by reverse creation time

            // let rp_id_hash = self.hash(rp_id.as_ref())?;

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

            let mut min_heap = MinCredentialHeap::new();

            // let mut credentials = CredentialList::new();

            let data = syscall!(self.crypto.read_dir_files_first(
                StorageLocation::Internal,
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
                Policy::Optional => true,
                Policy::OptionalWithCredentialIdList => allowed_credentials_passed || uv_performed,
                Policy::Required => uv_performed,
            };

            let kek = self.state.persistent.key_encryption_key(&mut self.crypto)?;

            if keep {
                let id = credential.id(&mut self.crypto, &kek)?;
                let credential_id_hash = self.hash(&id.0.as_ref())?;

                let timestamp_path = TimestampPath {
                    timestamp: credential.creation_time,
                    path: rk_path(&rp_id_hash, &credential_id_hash),
                };

                min_heap.push(timestamp_path).map_err(drop).unwrap();
                // hprintln!("first: {:?}", &self.hash(&id.0)).ok();
            }

            loop {
                let data = syscall!(self.crypto.read_dir_files_next()).data;
                let data = match data {
                    Some(data) => data,
                    None => break,
                };

                let credential = Credential::deserialize(&data).unwrap();

                let keep = match credential.cred_protect {
                    Policy::Optional => true,
                    Policy::OptionalWithCredentialIdList => allowed_credentials_passed || uv_performed,
                    Policy::Required => uv_performed,
                };

                if keep {

                    let id = credential.id(&mut self.crypto, &kek)?;
                    let credential_id_hash = self.hash(&id.0.as_ref())?;

                    let timestamp_path = TimestampPath {
                        timestamp: credential.creation_time,
                        path: rk_path(&rp_id_hash, &credential_id_hash),
                    };

                    // if credentials.len() == credentials.capacity() {
                    //     panic!("too many credentials! >{}", &credentials.len());
                    // }
                    // let id = credential.id(&mut self.crypto, &kek)?;
                    // credentials.push(credential).unwrap();
                    // hprintln!("next: {:?}", &self.hash(&id.0)).ok();

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

            // now sort them
            let max_heap = self.state.runtime.credential_heap();
            max_heap.clear();
            while !min_heap.is_empty() {
                max_heap.push(min_heap.pop().unwrap()).map_err(drop).unwrap();
            }

            None
        };

        //// apply credential protection policy
        //let applicable_credentials: CredentialList = existing_credentials
        //    .into_iter()
        //    .filter(|credential| {
        //        use credential::CredentialProtectionPolicy as Policy;
        //        debug!("CredentialProtectionPolicy {:?}", &credential.cred_protect).ok();
        //        match credential.cred_protect {
        //            Policy::Optional => true,
        //            Policy::OptionalWithCredentialIdList => allowed_credentials_passed || uv_performed,
        //            Policy::Required => uv_performed,

        //        }
        //    })
        //    .collect()
        //    ;
        ////

        // "If no applicable credentials were found, return CTAP2_ERR_NO_CREDENTIALS"
        if allowed_credential.is_none() && self.state.runtime.credential_heap().is_empty() {
            return Err(Error::NoCredentials);
        }

        Ok(allowed_credential)
    }

    fn get_next_assertion(&mut self) -> Result<ctap2::get_assertion::Response> {
        // 1./2. don't remember / don't have left any credentials
        if self.state.runtime.credential_heap().is_empty() {
            return Err(Error::NotAllowed);
        }

        // 3. previous GA/GNA >30s ago -> discard stat
        // this is optional over NFC
        if false {
            self.state.runtime.credential_heap().clear();
            return Err(Error::NotAllowed);
        }

        // 4. select credential
        let max_heap = self.state.runtime.credential_heap();
        let timestamp_hash = max_heap.pop().unwrap();
        hprintln!("{:?} @ {}", &timestamp_hash.path, timestamp_hash.timestamp).ok();
        let data = syscall!(self.crypto.read_file(
            StorageLocation::Internal,
            timestamp_hash.path,
        )).data;
        let credential = Credential::deserialize(&data).unwrap();

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

        let sub_parameters = &parameters.sub_command_params;
        match parameters.sub_command {

            // 0x1
            Subcommand::GetCredsMetadata => {
                self.verify_pin_auth_using_token(&parameters)?;
                cm::get_creds_metadata(self)
            }

            // 0x2
            Subcommand::EnumerateRpsBegin => {
                self.verify_pin_auth_using_token(&parameters)?;
                cm::enumerate_rps_begin(self)
            }

            // 0x3
            Subcommand::EnumerateRpsGetNextRp => {

                cm::enumerate_rps_get_next_rp(self)
            }

            // // 0x4
            // Subcommand::EnumerateCredentialsBegin => {

            //     self.verify_pin_auth_using_token(self, &parameters);
            //     cm::get_creds_metadata(
            //         self,
            //     )
            // }

            // // 0x5
            // Subcommand::EnumerateCredentialsGetNextCredential => {

            //     cm::get_creds_metadata(
            //         self,
            //     )
            // }

            // 0x6
            Subcommand::DeleteCredential => {
                self.verify_pin_auth_using_token(&parameters)?;
                let sub_parameters = sub_parameters.as_ref()
                    .ok_or(Error::MissingParameter)?;

                cm::delete_credential(
                    self,
                    // sub_parameters
                    //     .rp_id_hash.as_ref().ok_or(Error::MissingParameter)?,
                    sub_parameters
                        .credential_id.as_ref().ok_or(Error::MissingParameter)?,
                )
            }

            _ => todo!("not implemented yet"),
        }
    }

    fn get_assertion(&mut self, parameters: &ctap2::get_assertion::Parameters) -> Result<ctap2::get_assertion::Response> {

        let rp_id_hash = self.hash(&parameters.rp_id.as_ref())?;

        // 1-4.
        let uv_performed = self.pin_prechecks(
            &parameters.options, &parameters.pin_auth, &parameters.pin_protocol,
            &parameters.client_data_hash.as_ref(),
        )?;

        // 5. Locate eligible credentials
        //
        // Note: If allowList is passed, credential is Some(credential)
        // If no allowList is passed, credential is None and the retrieved credentials
        // are stored in state.runtime.credential_heap
        let credential = self.locate_credentials(&rp_id_hash, &parameters.allow_list, uv_performed)?;

        let (num_credentials, credential) = match credential {
            Some(credential) =>  (None, credential),
            None => {
                let max_heap = self.state.runtime.credential_heap();
                let timestamp_hash = max_heap.pop().unwrap();
                hprintln!("{:?} @ {}", &timestamp_hash.path, timestamp_hash.timestamp).ok();
                let data = syscall!(self.crypto.read_file(
                    StorageLocation::Internal,
                    timestamp_hash.path,
                )).data;
                let credential = Credential::deserialize(&data).unwrap();
                let num_credentials = match max_heap.len() {
                    0 => None,
                    n => Some(n as u32 + 1),
                };
                (num_credentials, credential)
            }
        };

        // NB: misleading, if we have "1" we return "None"
        debug!("found {:?} applicable credentials", num_credentials).ok();
        hprintln!("found {:?} applicable credentials", num_credentials).ok();

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
        });

        self.assert_with_credential(num_credentials, credential)
    }

    fn assert_with_credential(&mut self, num_credentials: Option<u32>, credential: Credential)
        -> Result<ctap2::get_assertion::Response>
    {
        let data = self.state.runtime.active_get_assertion.clone().unwrap();

        // 6. process any options present

        // 7. process any extensions present

        // 8. collect user presence

        // 9./10. sign clientDataHash || authData with "first" credential

        // hprintln!("signing with credential {:?}", &credential).ok();
        let kek = self.state.persistent.key_encryption_key(&mut self.crypto)?;
        let credential_id = credential.id(&mut self.crypto, &kek)?;

        use ctap2::AuthenticatorDataFlags as Flags;

        let sig_count = self.state.persistent.timestamp(&mut self.crypto)?;

        let authenticator_data = ctap2::get_assertion::AuthenticatorData {
            rp_id_hash: Bytes::try_from_slice(&data.rp_id_hash).unwrap(),

            flags: {
                let mut flags = Flags::USER_PRESENCE;
                if data.uv_performed {
                    flags |= Flags::USER_VERIFIED;
                }
                // if hmac_secret_requested.is_some() ||  cred_protect_requested != CredentialProtectionPolicy::Optional {
                //     flags |= Flags::EXTENSION_DATA;
                // }
                flags
            },

            sign_count: sig_count,
            attested_credential_data: None,
            extensions: None,
            // extensions: {
            //     parameters.extensions.clone()
            // },
        };

        let serialized_auth_data = authenticator_data.serialize();

        let mut commitment = Bytes::<consts::U1024>::new();
        commitment.extend_from_slice(&serialized_auth_data).map_err(|_| Error::Other)?;
        commitment.extend_from_slice(&data.client_data_hash).map_err(|_| Error::Other)?;

        let (key, gc) = match credential.key.clone() {
            Key::ResidentKey(key) => (key, false),
            Key::WrappedKey(bytes) => {
                let wrapping_key = self.state.persistent.key_wrapping_key(&mut self.crypto)?;
                // hprintln!("unwrapping {:?} with wrapping key {:?}", &bytes, &wrapping_key).ok();
                let key_result = syscall!(self.crypto.unwrap_key_chacha8poly1305(
                    &wrapping_key,
                    &bytes.try_convert_into().map_err(|_| Error::Other)?,
                    b"",
                    // &rp_id_hash,
                    StorageLocation::Volatile,
                )).key;
                // debug!("key result: {:?}", &key_result).ok();
                hprintln!("key result: {:?}", &key_result).ok();
                match key_result {
                    Some(key) => (key, true),
                    None => { return Err(Error::Other); }
                }
            }
        };

        let (mechanism, serialization) = match credential.algorithm {
            -7 => (Mechanism::P256, SignatureSerialization::Asn1Der),
            -8 => (Mechanism::Ed25519, SignatureSerialization::Raw),
            _ => { return Err(Error::Other); }
        };

        debug!("signing with {:?}, {:?}", &mechanism, &serialization).ok();
        let signature = syscall!(self.crypto.sign(mechanism, key.clone(), &commitment, serialization)).signature
            .try_convert_into().map_err(|_| Error::Other)?;
        if gc {
            syscall!(self.crypto.delete(key));
        }

        let response = ctap2::get_assertion::Response {
            credential: Some(credential_id.into()),
            // credential: None,
            auth_data: Bytes::try_from_slice(&serialized_auth_data).map_err(|_| Error::Other)?,
            signature,
            user: None,
            // number_of_credentials: if num_credentials > 1 { Some(num_credentials as u32) } else { None },
            number_of_credentials: num_credentials, //if num_credentials > 1 { Some(num_credentials as u32) } else { Some(1) },
        };

        Ok(response)
    }

    fn vendor(&mut self, op: VendorOperation) -> Result<()> {
        hprintln!("hello VO {:?}", &op).ok();
        match op.into() {
            0x79 => syscall!(self.crypto.debug_dump_store()),
            _ => return Err(Error::InvalidCommand),
        };

        Ok(())
    }

    // SECURITY considerations:
    // - we should "shred" the key material in crypto-service, not just delete it
    // - how to handle aborted/failed resets
    //
    // RELIABILITY/COMPLEXITY considerations:
    // - if it were just us, we could reformat (with shredding overwrite) the entire FS
    // - still, we could tell crypto-service to delete all our stuff,
    //   send our response,
    //   and then trigger a reboot
    fn reset(&mut self) -> Result<()> {
        // 1. >10s after bootup -> NotAllowed

        // 2. check for user presence
        // denied -> OperationDenied
        // timeout -> UserActionTimeout


        // DO IT

        // a. iterate over RKs, delete them
        // (can't just `remove_dir_all` as we need to delete all keys too!

        // may revisit the dir-walking API, but currently
        // we can only traverse one directory at once.
        loop {
            let dir = PathBuf::from(b"rk");

            hprintln!("reset start: reading {:?}", &dir).ok();
            let entry = syscall!(self.crypto.read_dir_first(
                StorageLocation::Internal,
                dir,
                None,
            )).entry;

            let rp_path = match entry {
                // no more RPs left
                None => break,
                Some(entry) => PathBuf::from(entry.path()),
            };
            hprintln!("got RP {:?}, looking for its RKs", &rp_path).ok();

            // delete all RKs for given RP

            let mut entry = syscall!(self.crypto.read_dir_first(
                StorageLocation::Internal,
                rp_path.clone(),
                None,
            )).entry;

            loop {
                // hprintln!("this may be an RK: {:?}", &entry).ok();
                let rk_path = match entry {
                    // no more RKs left
                    // break breaks inner loop here
                    None => break,
                    Some(entry) => PathBuf::from(entry.path()),
                };

                // prepare for next loop iteration
                entry = syscall!(self.crypto.read_dir_next(
                )).entry;

                self.delete_resident_key_by_path(&rk_path)?;
            }

            hprintln!("deleting RP dir {:?}", &rp_path).ok();
            syscall!(self.crypto.remove_dir(
                StorageLocation::Internal,
                rp_path,
            ));

        }

        // b. delete persistent state
        self.state.persistent.reset(&mut self.crypto)?;

        // c. delete runtime state
        self.state.runtime.reset(&mut self.crypto);

        // Missed anything?
        // One secret key remains currently, the fake attestation key

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
        hprintln!("deleting RK {:?}", &rk_path).ok();
        let credential_data = syscall!(self.crypto.read_file(
            StorageLocation::Internal,
            PathBuf::from(rk_path),
        )).data;
        let credential = Credential::deserialize(&credential_data).unwrap();
        // hprintln!("deleting credential {:?}", &credential).ok();

        match credential.key {
            credential::Key::ResidentKey(key) => {
                hprintln!(":: deleting resident key {:?}", &key).ok();
                syscall!(self.crypto.delete(key));
            }
            credential::Key::WrappedKey(_) => {}
        }

        if let Some(secret) = credential.hmac_secret.clone() {
            match secret {
                credential::CredRandom::Resident(secret) => {
                    hprintln!(":: deleting hmac secret {:?}", &secret).ok();
                    syscall!(self.crypto.delete(secret));
                }
                credential::CredRandom::Wrapped(_) => {}
            }
        }

        hprintln!(":: deleting RK file {:?} itself", &rk_path).ok();
        syscall!(self.crypto.remove_file(
            StorageLocation::Internal,
            PathBuf::from(rk_path),
        ));


        Ok(())
    }

    fn hash(&mut self, data: &[u8]) -> Result<Bytes<consts::U32>> {
        let hash = syscall!(self.crypto.hash_sha256(&data)).hash;
        hash.try_convert_into().map_err(|_| Error::Other)
    }

    fn make_credential(&mut self, parameters: &ctap2::make_credential::Parameters) -> Result<ctap2::make_credential::Response> {

        let rp_id_hash = self.hash(&parameters.rp.id.as_ref())?;

        // 1-4.
        let uv_performed = self.pin_prechecks(
            &parameters.options, &parameters.pin_auth, &parameters.pin_protocol,
            &parameters.client_data_hash.as_ref(),
        )?;

        // 5. "persist credProtect value for this credential"
        // --> seems out of place here, see 9.

        // 6. excludeList present, contains credential ID on this authenticator bound to RP?
        // --> wait for UP, error CredentialExcluded

        // 7. check pubKeyCredParams algorithm is valid + supported COSE identifier

        let mut algorithm: Option<SupportedAlgorithm> = None;
        for param in parameters.pub_key_cred_params.iter() {
            match param.alg {
                -7 => { if algorithm.is_none() { algorithm = Some(SupportedAlgorithm::P256); }}
                -8 => { algorithm = Some(SupportedAlgorithm::Ed25519); }
                _ => {}
            }
        }
        hprintln!("algo: {:?}", algorithm).ok();
        let algorithm = match algorithm {
            Some(algorithm) => algorithm,
            None => { return Err(Error::UnsupportedAlgorithm); }
        };
        // debug!("making credential, eddsa = {}", eddsa).ok();


        // 8. process options; on known but unsupported error UnsupportedOption

        let mut rk_requested = false;
        // TODO: why is this unused?
        let mut _uv_requested = false;
        let _up_requested = true; // can't be toggled

        info!("MC options: {:?}", &parameters.options).ok();
        if let Some(ref options) = &parameters.options {
            if Some(true) == options.rk {
                rk_requested = true;
            }
            if Some(true) == options.uv {
                _uv_requested = true;
            }
        }

        // 9. process extensions
        // TODO: need to figure out how to type-ify these
        // let mut hmac_secret_requested = false;
        // let mut cred_protect_requested = false;
        // if let Some(extensions) = &parameters.extensions {
        //     hmac_secret_requested = match extensions.get(&String::from("hmac-secret")) {
        //         Some(true) => true,
        //         _ => false,
        //     };

        //     cred_protect_requested = match extensions.get(&String::from("credProtect")) {
        //         Some(true) => true,
        //         _ => false,
        //     };
        // }

        let mut hmac_secret_requested = None;
        // let mut cred_protect_requested = CredentialProtectionPolicy::Optional;
        let mut cred_protect_requested = CredentialProtectionPolicy::default();
        if let Some(extensions) = &parameters.extensions {

            if let Some(true) = extensions.hmac_secret {
                // TODO: Generate "CredRandom" (a 32B random value, to be used
                // later via HMAC-SHA256(cred_random, salt)

                let cred_random = syscall!(self.crypto.generate_hmacsha256_key(
                    StorageLocation::Internal,
                )).key;

                hmac_secret_requested = Some(match rk_requested {
                    true => {
                        CredRandom::Resident(cred_random)
                    }

                    false => {
                        let wrapping_key = &self.state.persistent.key_wrapping_key(&mut self.crypto)?;
                        info!("wrapping credRandom").ok();
                        let wrapped_key = syscall!(self.crypto.wrap_key_chacha8poly1305(
                            &wrapping_key,
                            &cred_random,
                            // &rp_id_hash,
                            b"",
                        )).wrapped_key;

                        // 32B key, 12B nonce, 16B tag + some info on algorithm (P256/Ed25519)
                        // Turns out it's size 92 (enum serialization not optimized yet...)
                        // let mut wrapped_key = Bytes::<consts::U60>::new();
                        // wrapped_key.extend_from_slice(&wrapped_key_msg).unwrap();
                        CredRandom::Wrapped(wrapped_key.try_convert_into().map_err(|_| Error::Other)?)
                    }
                });
            }

            if let Some(policy) = &extensions.cred_protect {
                cred_protect_requested = CredentialProtectionPolicy::try_from(*policy)?;
            }
        }

        // debug!("hmac-secret = {:?}, credProtect = {:?}", hmac_secret_requested, cred_protect_requested).ok();

        // 10. get UP, if denied error OperationDenied
        if !self.up.user_present() {
            return Err(Error::OperationDenied);
        }

        // 11. generate credential keypair
        let location = match rk_requested {
            true => StorageLocation::Internal,
            false => StorageLocation::Volatile,
        };

        let private_key: ObjectHandle;
        let public_key: ObjectHandle;
        let cose_public_key;
        match algorithm {
            SupportedAlgorithm::P256 => {
                private_key = syscall!(self.crypto.generate_p256_private_key(location)).key;
                public_key = syscall!(self.crypto.derive_p256_public_key(&private_key, StorageLocation::Volatile)).key;
                cose_public_key = syscall!(self.crypto.serialize_key(
                    Mechanism::P256, public_key.clone(), KeySerialization::Cose
                )).serialized_key;

                info!("deleted public P256 key: {}", syscall!(self.crypto.delete(public_key)).success).ok();
            }
            SupportedAlgorithm::Ed25519 => {
                private_key = syscall!(self.crypto.generate_ed25519_private_key(location)).key;
                public_key = syscall!(self.crypto.derive_ed25519_public_key(&private_key, StorageLocation::Volatile)).key;
                cose_public_key = syscall!(self.crypto.serialize_key(
                    Mechanism::Ed25519, public_key.clone(), KeySerialization::Cose
                )).serialized_key;
                info!("deleted public Ed25519 key: {}", syscall!(self.crypto.delete(public_key)).success).ok();
            }
        }

        // 12. if `rk` is set, store or overwrite key pair, if full error KeyStoreFull

        // 12.a generate credential
        let key_parameter = match rk_requested {
            true => Key::ResidentKey(private_key.clone()),
            false => {
                // WrappedKey version
                let wrapping_key = &self.state.persistent.key_wrapping_key(&mut self.crypto)?;
                debug!("wrapping private key").ok();
                let wrapped_key = syscall!(self.crypto.wrap_key_chacha8poly1305(
                    &wrapping_key,
                    &private_key,
                    &rp_id_hash,
                )).wrapped_key;
                // debug!("wrapped_key = {:?}", &wrapped_key).ok();

                // 32B key, 12B nonce, 16B tag + some info on algorithm (P256/Ed25519)
                // Turns out it's size 92 (enum serialization not optimized yet...)
                // let mut wrapped_key = Bytes::<consts::U60>::new();
                // wrapped_key.extend_from_slice(&wrapped_key_msg).unwrap();
                let ret = Key::WrappedKey(wrapped_key.try_convert_into().map_err(|_| Error::Other)?);
                ret
                // debug!("len wrapped key = {}", wrapped_key.len()).ok();
                // Key::WrappedKey(wrapped_key.try_convert_into().unwrap())

            }
        };

        // injecting this is a bit mehhh..
        let nonce = syscall!(self.crypto.random_bytes(12)).bytes.as_ref().try_into().unwrap();
        info!("nonce = {:?}", &nonce).ok();

        let credential = Credential::new(
            credential::CtapVersion::Fido21Pre,
            parameters,
            algorithm as i32,
            key_parameter,
            self.state.persistent.timestamp(&mut self.crypto)?,
            hmac_secret_requested.clone(),
            cred_protect_requested,
            nonce,
        );

        // hprintln!("made credential {:?}", &credential).ok();

        // 12.b generate credential ID { = AEAD(Serialize(Credential)) }
        let kek = &self.state.persistent.key_encryption_key(&mut self.crypto)?;
        let credential_id = credential.id(&mut self.crypto, &kek)?;
        let credential_id_hash = self.hash(&credential_id.0.as_ref())?;

        // store it.
        // TODO: overwrite, error handling with KeyStoreFull

        let serialized_credential = credential.serialize()?;

        block!(self.crypto.write_file(
            StorageLocation::Internal,
            rk_path(&rp_id_hash, &credential_id_hash),
            serialized_credential.clone(),
            // user attribute for later easy lookup
            // Some(rp_id_hash.clone()),
            None,
        ).unwrap()).map_err(|_| Error::KeyStoreFull)?;

        // 13. generate and return attestation statement using clientDataHash

        // 13.a AuthenticatorData and its serialization
        use ctap2::AuthenticatorDataFlags as Flags;
        let authenticator_data = ctap2::make_credential::AuthenticatorData {
            rp_id_hash: rp_id_hash.try_convert_into().map_err(|_| Error::Other)?,

            flags: {
                let mut flags = Flags::USER_PRESENCE;
                if uv_performed {
                    flags |= Flags::USER_VERIFIED;
                }
                if true {
                    flags |= Flags::ATTESTED_CREDENTIAL_DATA;
                }
                if hmac_secret_requested.is_some() ||  cred_protect_requested != CredentialProtectionPolicy::Optional {
                    flags |= Flags::EXTENSION_DATA;
                }
                flags
            },

            sign_count: self.state.persistent.timestamp(&mut self.crypto)?,

            attested_credential_data: {
                // debug!("acd in, cid len {}, pk len {}", credential_id.0.len(), cose_public_key.len()).ok();
                let attested_credential_data = ctap2::make_credential::AttestedCredentialData {
                    aaguid: self.state.identity.aaguid(),
                    credential_id: credential_id.0.try_convert_into().unwrap(),
                    credential_public_key: cose_public_key.try_convert_into().unwrap(),
                };
                // debug!("cose PK = {:?}", &attested_credential_data.credential_public_key).ok();
                Some(attested_credential_data)
            },

            extensions: {
                parameters.extensions.clone()
            },
        };
        // debug!("authData = {:?}", &authenticator_data).ok();

        let serialized_auth_data = authenticator_data.serialize();

        // 13.b The Signature

        // can we write Sum<M, N> somehow?
        // debug!("seeking commitment, {} + {}", serialized_auth_data.len(), parameters.client_data_hash.len()).ok();
        let mut commitment = Bytes::<consts::U1024>::new();
        commitment.extend_from_slice(&serialized_auth_data).map_err(|_| Error::Other)?;
        // debug!("serialized_auth_data ={:?}", &serialized_auth_data).ok();
        commitment.extend_from_slice(&parameters.client_data_hash).map_err(|_| Error::Other)?;
        // debug!("client_data_hash = {:?}", &parameters.client_data_hash).ok();
        // debug!("commitment = {:?}", &commitment).ok();

        // NB: the other/normal one is called "basic" or "batch" attestation,
        // because it attests the authenticator is part of a batch: the model
        // specified by AAGUID.
        // "self signed" is also called "surrogate basic".
        //
        // we should also directly support "none" format, it's a bit weird
        // how browsers firefox this
        const SELF_SIGNED: bool  = true;

        let (signature, attestation_algorithm) = {
            if SELF_SIGNED {
                match algorithm {
                    SupportedAlgorithm::Ed25519 => {
                        let signature = syscall!(self.crypto.sign_ed25519(&private_key, &commitment)).signature;
                        (signature.try_convert_into().map_err(|_| Error::Other)?, -8)
                    }

                    SupportedAlgorithm::P256 => {
                        // DO NOT prehash here, `crypto-service` does that
                        let der_signature = syscall!(self.crypto.sign_p256(&private_key, &commitment, SignatureSerialization::Asn1Der)).signature;
                        (der_signature.try_convert_into().map_err(|_| Error::Other)?, -7)
                    }
                }
            } else {
                let hash = syscall!(self.crypto.hash_sha256(&commitment.as_ref())).hash;
                let attestation_key = self.state.identity.attestation_key(&mut self.crypto);
                let signature = syscall!(self.crypto.sign_p256(
                    &attestation_key,
                    &hash,
                    SignatureSerialization::Asn1Der,
                )).signature;
                (signature.try_convert_into().map_err(|_| Error::Other)?, -7)
            }
        };
        // debug!("SIG = {:?}", &signature).ok();

        if !rk_requested {
            info!("deleted private credential key: {}", syscall!(self.crypto.delete(private_key)).success).ok();
        }

        let packed_attn_stmt = ctap2::make_credential::PackedAttestationStatement {
            alg: attestation_algorithm,
            sig: signature,
            x5c: match SELF_SIGNED {
                true => None,
                false => {
                    // let mut x5c = Vec::new();
                    // x5c.push(Bytes::try_from_slice(&SOLO_HACKER_ATTN_CERT).unwrap()).unwrap();
                    //
                    // See: https://www.w3.org/TR/webauthn-2/#sctn-packed-attestation-cert-requirements
                    //
                    todo!("solve the cert conundrum");
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
    //     // let unwrapped_key = syscall!(self.crypto.unwrap_key_chacha8poly1305(
    //     //     &wrapping_key,
    //     //     &wrapped_key,
    //     //     b"",
    //     //     StorageLocation::Volatile,
    //     // )).key;
        // // test public key ser/de
        // let ser_pk = syscall!(self.crypto.serialize_key(
        //     Mechanism::P256, public_key.clone(), KeySerialization::Raw
        // )).serialized_key;
        // debug!("ser pk = {:?}", &ser_pk).ok();

        // let cose_ser_pk = syscall!(self.crypto.serialize_key(
        //     Mechanism::P256, public_key.clone(), KeySerialization::Cose
        // )).serialized_key;
        // debug!("COSE ser pk = {:?}", &cose_ser_pk).ok();

        // let deser_pk = syscall!(self.crypto.deserialize_key(
        //     Mechanism::P256, ser_pk.clone(), KeySerialization::Raw,
        //     StorageAttributes::new().set_persistence(StorageLocation::Volatile)
        // )).key;
        // debug!("deser pk = {:?}", &deser_pk).ok();

        // let cose_deser_pk = syscall!(self.crypto.deserialize_key(
        //     Mechanism::P256, cose_ser_pk.clone(), KeySerialization::Cose,
        //     StorageAttributes::new().set_persistence(StorageLocation::Volatile)
        // )).key;
        // debug!("COSE deser pk = {:?}", &cose_deser_pk).ok();
        // debug!("raw ser of COSE deser pk = {:?}",
        //           syscall!(self.crypto.serialize_key(Mechanism::P256, cose_deser_pk.clone(), KeySerialization::Raw)).
        //           serialized_key).ok();

        // debug!("priv {:?}", &private_key).ok();
        // debug!("pub {:?}", &public_key).ok();

        // let _loaded_credential = syscall!(self.crypto.load_blob(
        //     prefix.clone(),
        //     blob_id,
        //     StorageLocation::Volatile,
        // )).data;
        // // debug!("loaded credential = {:?}", &loaded_credential).ok();

        // debug!("credential = {:?}", &Credential::deserialize(&serialized_credential)?).ok();

    //     // debug!("unwrapped_key = {:?}", &unwrapped_key).ok();

    fn get_info(&mut self) -> ctap2::get_info::Response {

        use core::str::FromStr;
        let mut versions = Vec::<String<consts::U12>, consts::U3>::new();
        versions.push(String::from_str("FIDO_2_1_PRE").unwrap()).unwrap();
        versions.push(String::from_str("FIDO_2_0").unwrap()).unwrap();
        versions.push(String::from_str("U2F_V2").unwrap()).unwrap();

        let mut extensions = Vec::<String<consts::U11>, consts::U4>::new();
        extensions.push(String::from_str("hmac-secret").unwrap()).unwrap();
        extensions.push(String::from_str("credProtect").unwrap()).unwrap();

        let mut pin_protocols = Vec::<u8, consts::U1>::new();
        pin_protocols.push(1).unwrap();

        let mut options = ctap2::get_info::CtapOptions::default();
        options.rk = true;
        options.up = true;
        options.uv = None; // "uv" here refers to "in itself", e.g. biometric
        // options.plat = false;
        // options.client_pin = None; // not capable of PIN
        options.client_pin = match self.state.persistent.pin_is_set() {
            true => Some(true),
            false => None,
        };
        // options.client_pin = Some(true/false); // capable, is set/is not set

        ctap2::get_info::Response {
            versions,
            extensions: Some(extensions),
            aaguid: self.state.identity.aaguid(),
            options: Some(options),
            max_msg_size: Some(ctap_types::sizes::MESSAGE_SIZE),
            pin_protocols: Some(pin_protocols),
            ..ctap2::get_info::Response::default()
        }
    }

//     fn get_or_create_counter_handle(crypto_client: &mut CryptoClient) -> Result<ObjectHandle> {

//         // there should be either 0 or 1 counters with this name. if not, it's a logic error.
//         let attributes = Attributes {
//             kind: Counter,
//             label: Self::GLOBAL_COUNTER_NAME.into(),
//         };

//         // let reply = syscall!(FindObjects, attributes)?;

//         let reply = block!(
//             request::FindObjects { attributes }
//                 .submit(&mut crypto_client)
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
//                         .submit(&mut crypto_client)
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

//     fn get_or_create_counter_handle(crypto_client: &mut CryptoClient) -> Result<ObjectHandle> {
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
