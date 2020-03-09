 #![cfg_attr(not(test), no_std)]

use core::task::Poll;
use core::convert::{TryFrom, TryInto};

use cortex_m_semihosting::hprintln;

use crypto_service::{
    Client as CryptoClient,
    pipe::Syscall as CryptoSyscall,
    types::{
        KeySerialization,
        Mechanism,
        ObjectHandle,
        StorageLocation,
        StorageAttributes,
    },
};
use ctap_types::{
    Bytes, consts, String, Vec,
    rpc::AuthenticatorEndpoint,
    authenticator::{ctap1, ctap2, Error, Request, Response},
};

pub mod credential;
pub use credential::*;

type Result<T> = core::result::Result<T, Error>;

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

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Configuration {
    aaguid: Bytes<consts::U16>,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct State {
    counter: Option<ObjectHandle>,
    key_agreement_key: Option<ObjectHandle>,
    key_encryption_key: Option<ObjectHandle>,
    key_wrapping_key: Option<ObjectHandle>,
    pin_token: Option<ObjectHandle>,
    retries: Option<u8>,
    pin_hash: Option<[u8; 16]>,
}

// impl State {
//     pub fn key_agreement_key(crypto: &mut CryptoClient
// }

pub struct Authenticator<'a, S, UP>
where
    S: CryptoSyscall,
    UP: UserPresence,
{
    config: Configuration,
    crypto: CryptoClient<'a, S>,
    rpc: AuthenticatorEndpoint<'a>,
    state: State,
    up: UP,
}

// #[derive(Clone, Debug)]
// pub enum Error {
//     Catchall,
// }

macro_rules! block {
    ($future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $future_result;
        loop {
            match future_result.poll() {
                Poll::Ready(result) => { break result; },
                Poll::Pending => {},
            }
        }
    }}
}

macro_rules! syscall {
    ($pre_future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $pre_future_result.expect("no client error");
        loop {
            match future_result.poll() {
                Poll::Ready(result) => { break result.expect("no errors"); },
                Poll::Pending => {},
            }
        }
    }}
}

impl<'a, S: CryptoSyscall, UP: UserPresence> Authenticator<'a, S, UP> {

    pub fn new(mut crypto: CryptoClient<'a, S>, rpc: AuthenticatorEndpoint<'a>, up: UP) -> Self {

        let config = Configuration {
            aaguid: Bytes::try_from_slice(b"AAGUID0123456789").unwrap(),
        };
        let state = State::default();
        let authenticator = Authenticator { config, crypto, rpc, state, up };

        authenticator
    }

    pub fn key_agreement_key(&mut self) -> Result<ObjectHandle> {
        match self.state.key_agreement_key.clone() {
            Some(key) => Ok(key),
            None => self.rotate_key_agreement_key(),
        }
    }

    pub fn rotate_key_encryption_key(&mut self) -> Result<ObjectHandle> {
        let key = block!(self.crypto
            .generate_chacha8poly1305_key(StorageLocation::Volatile).map_err(|_| Error::Other)?)
            .map_err(|_| Error::Other)?.key;
        self.state.key_encryption_key = Some(key.clone());
        Ok(key)
    }

    pub fn key_encryption_key(&mut self) -> Result<ObjectHandle> {
        match self.state.key_encryption_key.clone() {
            Some(key) => Ok(key),
            None => self.rotate_key_encryption_key(),
        }
    }

    pub fn rotate_key_agreement_key(&mut self) -> Result<ObjectHandle> {
        let key = block!(self.crypto
            .generate_p256_private_key(StorageLocation::Volatile).map_err(|_| Error::Other)?)
            .map_err(|_| Error::Other)?.key;
        self.state.key_agreement_key = Some(key.clone());
        Ok(key)
    }

    pub fn retries(&mut self) -> Result<u8> {
        match self.state.retries {
            Some(retries) => Ok(retries),
            None => {
                self.state.retries = Some(8);
                Ok(8)
            }
        }
    }

    pub fn pin_token(&mut self) -> Result<ObjectHandle> {
        match self.state.pin_token.clone() {
            Some(key) => Ok(key),
            None => self.rotate_pin_token(),
        }
    }

    pub fn rotate_pin_token(&mut self) -> Result<ObjectHandle> {
        let key = syscall!(self.crypto.generate_hmacsha256_key(StorageLocation::Volatile)).key;
        self.state.pin_token = Some(key.clone());
        Ok(key)
    }

    pub fn pin_is_set(&self) -> bool {
        self.state.pin_hash.is_some()
    }

    // pub(crate) fn config(&mut self) -> Result<C
    //     Err(Error::Initialisation)
    // }

    fn respond(&mut self, response: Result<Response>) {
        self.rpc.send.enqueue(response).expect("internal error");
    }

    pub fn poll(&mut self) {
        let kek = self.key_agreement_key().unwrap();
        // hprintln!("polling authnr, kek = {:?}", &kek).ok();

        match self.rpc.recv.dequeue() {
            None => {},
            Some(request) => {
                // hprintln!("request: {:?}", &request).ok();

                use ctap_types::authenticator::{Error, Request, Response};

                match request {
                    Request::Ctap2(request) => {
                        match request {
                            ctap2::Request::GetInfo => {
                                let response = self.get_info();
                                self.rpc.send.enqueue(
                                    Ok(Response::Ctap2(ctap2::Response::GetInfo(response))))
                                    .expect("internal error");
                            }
                            // 0x1
                            ctap2::Request::MakeCredential(parameters) => {
                                // hprintln!("MC: {:?}", &parameters).ok();
                                let response = self.make_credential(&parameters);
                                self.rpc.send.enqueue(
                                    match response {
                                        Ok(response) => Ok(Response::Ctap2(ctap2::Response::MakeCredential(response))),
                                        Err(error) => Err(error)
                                    })
                                    .expect("internal error");
                                hprintln!("enqueued MC response").ok();
                            }

                            // 0x6
                            ctap2::Request::ClientPin(parameters) => {
                                let response = self.client_pin(&parameters);
                                self.rpc.send.enqueue(
                                    match response {
                                        Ok(response) => Ok(Response::Ctap2(ctap2::Response::ClientPin(response))),
                                        Err(error) => Err(error)
                                    })
                                    .expect("internal error");
                                hprintln!("enqueued CP response").ok();
                            }
                            _ => {
                                hprintln!("not implemented: {:?}", &request).ok();
                                self.rpc.send.enqueue(Err(Error::InvalidCommand)).expect("internal error");
                            }
                        }
                    }
                    Request::Ctap1(request) => {
                        hprintln!("ctap1 not implemented: {:?}", &request).ok();
                        // self.rpc.send.enqueue(Err(Error::InvalidCommand)).expect("internal error");
                        self.respond(Err(Error::InvalidCommand));
                    }
                }
            }
        }
    }

    // fn verify_pin(&mut self, pin_auth: &Bytes<consts::U16>, client_data_hash: &Bytes<consts::U32>) -> bool {
    fn verify_pin(&mut self, pin_auth: &[u8; 16], data: &[u8]) -> bool {
        let key = self.pin_token().unwrap();
        let tag = syscall!(self.crypto.sign_hmacsha256(&key, data)).signature;
        pin_auth == &tag[..16]
    }

    fn client_pin(&mut self, parameters: &ctap2::client_pin::Parameters) -> Result<ctap2::client_pin::Response> {
        use ctap2::client_pin::PinV1Subcommand as Subcommand;
        hprintln!("processing CP").ok();

        if parameters.pin_protocol != 1{
            return Err(Error::InvalidParameter);
        }

        Ok(match parameters.sub_command {

            Subcommand::GetRetries => {
                hprintln!("processing CP.GR").ok();
                ctap2::client_pin::Response {
                    key_agreement: None,
                    pin_token: None,
                    retries: Some(self.retries().unwrap()),
                }

            }

            Subcommand::GetKeyAgreement => {
                let private_key = self.key_agreement_key().unwrap();
                let public_key = syscall!(self.crypto.derive_p256_public_key(&private_key, StorageLocation::Volatile)).key;
                hprintln!("processing CP.GKA").ok();

                todo!();
            }

            Subcommand::SetPin => {
                // check mandatory parameters
                let platform_key_agreement_key = match parameters.key_agreement.as_ref() {
                    Some(key) => key,
                    None => { return Err(Error::MissingParameter); }
                };
                let new_pin_enc = match parameters.new_pin_enc.as_ref() {
                    Some(pin) => pin,
                    None => { return Err(Error::MissingParameter); }
                };
                let pin_auth = match parameters.pin_auth.as_ref() {
                    Some(pin) => pin,
                    None => { return Err(Error::MissingParameter); }
                };

                // is pin already set
                if self.pin_is_set() {
                    return Err(Error::PinAuthInvalid);
                }

                // generate shared secret
                // // deserialize passed public key in crypto service
                // let public_key = syscall!(self.crypto.create_p256_public_key(
                //         &platform_key_agreement_key, StorageLocation::Volatile)).key;
                // let agreement = syscall!(self.crypto.agree_p256(
                //         &self.key_agreement_key().unwrap(), &public_key, StorageLocation::Volatile)).shared_secret;
                // let shared_secret = syscall!(self.crypto.derive_key(
                //         Mechanism::Sha256, agreement, StorageLocation::Volatile)).key;

                // verify pinAuth (can we use self.verify_pin??)
                // let verifies = {
                //     let tag = syscall!(self.crypto.sign_hmacsha256(&shared_secret, new_pin_enc)).signature;
                //     pin_auth == &tag[..16]
                // };
                // if !verifies {
                //     return Err(Error::PinAuthInvalid);
                // }

                // decrypt newPin using shared secret, check minimum length of 4 bytes
                // NB: platform pads pin with 0x0 bytes, find first...
                return Err(Error::PinPolicyViolation);

                // store LEFT(Sha256(newPin), 16) on device

                todo!();
            }

            Subcommand::ChangePin => {
                todo!();
            }

            Subcommand::GetPinToken => {
                todo!();
            }

        })
    }

    fn make_credential(&mut self, parameters: &ctap2::make_credential::Parameters) -> Result<ctap2::make_credential::Response> {

        // 1. pinAuth zero length -> wait for user touch, then
        // return PinNotSet if not set, PinInvalid if set
        //
        // the idea is for multi-authnr scenario where platform
        // wants to enforce PIN and needs to figure out which authnrs support PIN
        if let Some(ref pin_auth) = &parameters.pin_auth {
            if pin_auth.len() == 0 {
                if !self.up.user_present() {
                    return Err(Error::OperationDenied);
                }
                if !self.pin_is_set() {
                    return Err(Error::PinNotSet);
                } else {
                    return Err(Error::PinAuthInvalid);
                }

            }
        }

        // 2. check PIN protocol is 1 if pinAuth was sent
        if let Some(ref pin_auth) = &parameters.pin_auth {
            if let Some(1) = parameters.pin_protocol {
            } else {
                return Err(Error::PinAuthInvalid);
            }
        }

        // 3. if no PIN is set (we have no other form of UV),
        // and platform sent `uv` or `pinAuth`, return InvalidOption
        if !self.pin_is_set() {
            if let Some(ref options) = &parameters.options {
                if Some(true) == options.uv {
                    return Err(Error::InvalidOption);
                }
            }
            if parameters.pin_auth.is_some() {
                return Err(Error::InvalidOption);
            }
        }

        // 4. TODO: move pinAuth up here
        // Also clarify the confusion... I think we should fail if `uv` is passed?
        // As we don't have our "own" gesture such as fingerprint or on-board PIN entry?


        // 5. credProtect?

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
        let algorithm = match algorithm {
            Some(algorithm) => algorithm,
            None => { return Err(Error::UnsupportedAlgorithm); }
        };
        // hprintln!("making credential, eddsa = {}", eddsa).ok();


        // 8. process options; on known but unsupported error UnsupportedOption

        let mut rk_requested = false;
        let mut uv_requested = false;
        let mut up_requested = true; // can't be toggled

        if let Some(ref options) = &parameters.options {
            if Some(true) == options.rk {
                rk_requested = true;
            }
            if Some(true) == options.uv {
                uv_requested = true;
            }
        }

        // 9. process extensions
        // TODO: need to figure out how to type-ify these


        // "old" (CTAP2, not CTAP2.1): 5., 6., 7. pinAuth handling
        // TODO: move up

        let mut uv_performed = false;
        if let Some(ref pin_auth) = &parameters.pin_auth {
            if pin_auth.len() != 16 {
                return Err(Error::InvalidParameter);
            }
            if let Some(1) = parameters.pin_protocol {
                // 5. if pinAuth is present and pinProtocol = 1, verify
                // success --> set uv = 1
                // error --> PinAuthInvalid
                uv_performed = self.verify_pin(
                    // unwrap panic ruled out above
                    pin_auth.as_ref().try_into().unwrap(),
                    &parameters.client_data_hash.as_ref(),
                );
                if !uv_performed {
                    return Err(Error::PinAuthInvalid);
                }

            } else {
                // 7. pinAuth present + pinProtocol != 1 --> error PinAuthInvalid
                return Err(Error::PinAuthInvalid);
            }

        } else {
            // 6. pinAuth not present + clientPin set --> error PinRequired
            if self.pin_is_set() {
                return Err(Error::PinRequired);
            }
        }

        // 10. get UP, if denied error OperationDenied
        if !self.up.user_present() {
            return Err(Error::OperationDenied);
        }

        // 11. generated credential keypair

        let location = match rk_requested {
            true => StorageLocation::Internal,
            false => StorageLocation::Volatile,
        };

        let mut private_key: ObjectHandle;
        let mut public_key: ObjectHandle;
        match algorithm {
            SupportedAlgorithm::P256 => {
                private_key = syscall!(self.crypto.generate_p256_private_key(location)).key;
                public_key = syscall!(self.crypto.derive_p256_public_key(&private_key, StorageLocation::Volatile)).key;
            }
            SupportedAlgorithm::Ed25519 => {
                private_key = syscall!(self.crypto.generate_ed25519_private_key(location)).key;
                public_key = syscall!(self.crypto.derive_ed25519_public_key(&private_key, StorageLocation::Volatile)).key;
            }
        }

        // test public key ser/de
        let ser_pk = syscall!(self.crypto.serialize_key(
            Mechanism::P256, public_key.clone(), KeySerialization::Raw
        )).serialized_key;
        hprintln!("ser pk = {:?}", &ser_pk).ok();

        let deser_pk = syscall!(self.crypto.deserialize_key(
            Mechanism::P256, ser_pk.clone(), KeySerialization::Raw,
            StorageAttributes::new().set_persistence(StorageLocation::Volatile)
        )).key;
        hprintln!("deser pk = {:?}", &deser_pk).ok();

        // hprintln!("priv {:?}", &private_key).ok();
        // hprintln!("pub {:?}", &public_key).ok();

        // TODO: add wrapped key
        // let key_parameter = match rk_requested {
        //     true => Some(private_key.clone()),
        //     false => None,
        // };
        let key_parameter = credential::Key::ResidentKey(private_key.clone());

        let credential = Credential::new(
            credential::CtapVersion::Fido21Pre,
            parameters,
            algorithm as i32,
            key_parameter,
            123, // todo: get counter
            false, // todo: hmac-secret?
            false, // todo: cred-protect?
        );
        // hprintln!("credential = {:?}", &credential).ok();

        // 12. if `rk` is set, store or overwrite key pair, if full error KeyStoreFull
        // e.g., 44B
        let serialized_credential = credential.serialize()?;
        // hprintln!("serialized credential = {:?}", &serialized_credential).ok();

        let mut prefix = crypto_service::types::ShortData::new();
        prefix.extend_from_slice(b"rk").map_err(|_| Error::Other)?;
        let prefix = Some(crypto_service::types::Letters::try_from(prefix).map_err(|_| Error::Other)?);
        let blob_id = syscall!(self.crypto.store_blob(
            prefix.clone(),
            // credential_id.0.clone(),
            serialized_credential.clone(),
            StorageLocation::Volatile,
        )).blob;

        let loaded_credential = syscall!(self.crypto.load_blob(
            prefix.clone(),
            blob_id,
            StorageLocation::Volatile,
        )).data;
        // hprintln!("loaded credential = {:?}", &loaded_credential).ok();

        // hprintln!("credential = {:?}", &Credential::deserialize(&serialized_credential)?).ok();

        let key = &self.key_encryption_key()?;
        let message = &serialized_credential;
        let associated_data = parameters.rp.id.as_bytes();
        let encrypted_serialized_credential = EncryptedSerializedCredential(
            syscall!(self.crypto.encrypt_chacha8poly1305(key, message, associated_data)));

        // hprintln!("esc = {:?}", &encrypted_serialized_credential).ok();
        // e.g., 72B
        let credential_id: CredentialId = encrypted_serialized_credential.try_into().unwrap();
        // hprintln!("cid = {:?}", &credential_id).ok();
        // hprintln!("credential_id.len() = {}", credential_id.0.len()).ok();

        let esc: EncryptedSerializedCredential = credential_id.try_into().unwrap();
        // hprintln!("esc = {:?}", &esc).ok();


        // WrappedKey version
        let wrapping_key = &self.key_encryption_key()?;
        let wrapped_key = syscall!(self.crypto.wrap_key_chacha8poly1305(
            &wrapping_key,
            &private_key,
            b"",
        )).wrapped_key;
        hprintln!("wrapped_key = {:?}", &wrapped_key).ok();

        let unwrapped_key = syscall!(self.crypto.unwrap_key_chacha8poly1305(
            &wrapping_key,
            &wrapped_key,
            b"",
            StorageLocation::Volatile,
        )).key;
        hprintln!("unwrapped_key = {:?}", &unwrapped_key).ok();

        // 13. generate and return attestation statement using clientDataHash

        hprintln!("MC NOT FINISHED YET").ok();
        Err(Error::Other)

        // ctap2::make_credential::Response {
        //     versions,
        //     aaguid: self.config.aaguid.clone(),
        //     max_msg_size: Some(ctap_types::sizes::MESSAGE_SIZE),
        //     ..ctap2::get_info::Response::default()
        // }
    }

    // fn credential_id(credential: &Credential) -> CredentialId {
    // }

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
        options.client_pin = None; // not capable of PIN
        // options.client_pin = Some(true/false); // capable, is set/is not set

        ctap2::get_info::Response {
            versions,
            extensions: Some(extensions),
            aaguid: self.config.aaguid.clone(),
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
