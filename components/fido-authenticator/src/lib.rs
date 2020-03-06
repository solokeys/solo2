 // #![cfg_attr(not(test), no_std)]
#![no_std]

use cortex_m_semihosting::hprintln;

use core::task::Poll;
use core::convert::TryInto;

use crypto_service::{
    Client as CryptoClient,
    pipe::Syscall as CryptoSyscall,
    types::{
        ObjectHandle,
        StorageLocation,
    },
};
use ctap_types::{
    Bytes, consts, String, Vec,
    rpc::AuthenticatorEndpoint,
    authenticator::{ctap1, ctap2, Error as CtapError},
};

// use usbd_ctaphid::{
//     authenticator::{
//         self,
//         Error,
//         Result,
//     },
//     types::*,
// };

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Configuration {
    aaguid: Bytes<consts::U16>,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct State {
    counter: Option<ObjectHandle>,
    key_agreement_key: Option<ObjectHandle>,
    key_wrapping_key: Option<ObjectHandle>,
    client_pin_set: bool,
    // pin_hash: Option<>,
}

// impl State {
//     pub fn key_agreement_key(crypto: &mut CryptoClient
// }

pub struct Authenticator<'a, S>
where
    S: CryptoSyscall,
{
    config: Configuration,
    crypto: CryptoClient<'a, S>,
    rpc: AuthenticatorEndpoint<'a>,
    state: State,

}

#[derive(Clone, Debug)]
pub enum Error {
    Catchall,
    Initialisation,
}

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

impl<'a, S: CryptoSyscall> Authenticator<'a, S> {

    pub fn new(mut crypto: CryptoClient<'a, S>, rpc: AuthenticatorEndpoint<'a>) -> Self {

        let config = Configuration {
            aaguid: Bytes::try_from_slice(b"AAGUID0123456789").unwrap(),
        };
        let state = State::default();
        let authenticator = Authenticator { config, crypto, rpc, state };

        authenticator
    }

    pub fn key_agreement_key(&mut self) -> Result<ObjectHandle, Error> {
        match self.state.key_agreement_key {
            Some(key) => Ok(key),
            None => {
                let key = block!(self.crypto
                    .generate_p256_private_key(StorageLocation::Volatile).map_err(|_| Error::Catchall)?)
                    .map_err(|_| Error::Catchall)?.key;
                self.state.key_agreement_key = Some(key);
                Ok(key)
            }
        }
    }

    // pub(crate) fn config(&mut self) -> Result<C
    //     Err(Error::Initialisation)
    // }

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
                            _ => {
                                hprintln!("not implemented: {:?}", &request).ok();
                                self.rpc.send.enqueue(Err(Error::InvalidCommand)).expect("internal error");
                            }
                        }
                    }
                    Request::Ctap1(request) => {
                    }
                }
            }
        }
    }

    fn make_credential(&mut self, parameters: &ctap2::make_credential::Parameters) -> Result<ctap2::make_credential::Response, CtapError> {

        // 1. excludeList present, contains credential ID on this authenticator bound to RP?
        // --> wait for UP, error CredentialExcluded


        // 2. check pubKeyCredParams algorithm is valid + supported COSE identifier

        let mut supported_algorithm = false;
        let mut eddsa = false;
        for param in parameters.pub_key_cred_params.iter() {
            match param.alg {
                -7 => { supported_algorithm = true; },
                -8 => { eddsa = true; supported_algorithm = true; },
                _ => {},
            }
        }
        if !supported_algorithm {
            return Err(CtapError::UnsupportedAlgorithm);
        }
        // hprintln!("making credential, eddsa = {}", eddsa).ok();


        // 3. process options; on known but unsupported error UnsupportedOption

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

        // 4. process extensions
        // TODO: need to figure out how to type-ify these


        // 5., 6., 7. pinAuth handling

        let mut uv_performed = false;
        if let Some(ref pin_auth) = &parameters.pin_auth {
            if let Some(1) = parameters.pin_protocol {
                // 5. if pinAuth is present and pinProtocol = 1, verify
                // success --> set uv = 1
                // error --> PinAuthInvalid
                uv_performed = self.verify_pin(
                    pin_auth.as_ref().try_into().unwrap(),
                    &parameters.client_data_hash.as_ref().try_into().unwrap(),
                );
                if !uv_performed {
                    return Err(CtapError::PinAuthInvalid);
                }

            } else {
                // 7. pinAuth present + pinProtocol != 1 --> error PinAuthInvalid
                return Err(CtapError::PinAuthInvalid);
            }

        } else {
            // 6. pinAuth not present + clientPin set --> error PinRequired
            if self.state.client_pin_set {
                return Err(CtapError::PinRequired);
            }
        }

        // 8. get UP, if denied error OperationDenied

        // 9. generated credential keypair

        let mut private_key: ObjectHandle;
        let mut public_key: ObjectHandle;
        if eddsa {
            todo!("eddsa MC not implemented");
        } else {
            let location = match rk_requested {
                true => StorageLocation::Internal,
                false => StorageLocation::Volatile,
            };

            private_key = syscall!(self.crypto.generate_p256_private_key(location)).key;
            hprintln!("priv {:?}", &private_key).ok();

            public_key = syscall!(self.crypto.derive_p256_public_key(&private_key, StorageLocation::Volatile))
                .key;
            hprintln!("pub {:?}", &public_key).ok();
        }

        // 10. if `rk`, store or overwrite key pair, if full error KeyStoreFull

        // 11. generate and return attestation statement using clientDataHash

        hprintln!("MC NOT FINISHED YET").ok();
        Err(CtapError::Other)

        // ctap2::make_credential::Response {
        //     versions,
        //     aaguid: self.config.aaguid.clone(),
        //     max_msg_size: Some(ctap_types::sizes::MESSAGE_SIZE),
        //     ..ctap2::get_info::Response::default()
        // }
    }

    // fn verify_pin(&mut self, pin_auth: &Bytes<consts::U16>, client_data_hash: &Bytes<consts::U32>) -> bool {
    fn verify_pin(&mut self, pin_auth: &[u8; 16], client_data_hash: &[u8; 32]) -> bool {
        // let _tag = block!(
        //     client.sign(Mechanism::HmacSha256, symmetric_key.clone(), &new_pin_enc)
        //         .expect("no client error"))
        //     .expect("no errors").signature;
        false
    }

    fn get_info(&mut self) -> ctap2::get_info::Response {

        use core::str::FromStr;
        let mut versions = Vec::<String<consts::U12>, consts::U3>::new();
        versions.push(String::from_str("FIDO_2_1_PRE").unwrap()).unwrap();
        versions.push(String::from_str("FIDO_2_0").unwrap()).unwrap();
        versions.push(String::from_str("U2F_V2").unwrap()).unwrap();

        ctap2::get_info::Response {
            versions,
            aaguid: self.config.aaguid.clone(),
            max_msg_size: Some(ctap_types::sizes::MESSAGE_SIZE),
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
