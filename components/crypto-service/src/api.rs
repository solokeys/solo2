//! This (incomplete!) API loosely follows [PKCS#11 v3][pkcs11-v3].
//!
//! For constants see [their headers][pkcs11-headers].
//!
//! [pkcs11-v3]: https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.0/pkcs11-base-v3.0.html
//! [pkcs11-headers]: https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.0/cs01/include/pkcs11-v3.0/

use crate::types::*;

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Request {
    DummyRequest, // for testing
    GenerateKey(request::GenerateKey),
    GenerateKeypair(request::GenerateKeypair),
    Sign(request::Sign),
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Reply {
    DummyReply, // for testing
    GenerateKey(reply::GenerateKey),
    GenerateKeypair(reply::GenerateKeypair),
    Sign(reply::Sign),
}

pub mod request {
    use super::*;

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct GenerateKey {
        pub mechanism: Mechanism,
        pub key_parameters: KeyParameters,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct GenerateKeypair {
        pub mechanism: Mechanism,
        pub key_parameters: KeyParameters,
        // private_key_template: PrivateKeyTemplate,
        // public_key_template: PublicKeyTemplate,
    }

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct Sign {
        pub key_handle: KeyHandle,
        pub mechanism: Mechanism,
        pub message: Message,
    }

    impl From<request::GenerateKey> for Request {
        fn from(request: request::GenerateKey) -> Self {
            Self::GenerateKey(request)
        }
    }

    impl From<request::GenerateKeypair> for Request {
        fn from(request: request::GenerateKeypair) -> Self {
            Self::GenerateKeypair(request)
        }
    }

    impl From<request::Sign> for Request {
        fn from(request: request::Sign) -> Self {
            Self::Sign(request)
        }
    }

}

pub mod reply {
    use super::*;

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct GenerateKey {
        pub key_handle: KeyHandle,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct GenerateKeypair {
        pub public_key_handle: KeyHandle,
        pub private_key_handle: KeyHandle,
    }

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct Sign {
        pub signature: Signature,
    }

}
