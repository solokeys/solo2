//! This (incomplete!) API loosely follows [PKCS#11 v3][pkcs11-v3].
//!
//! For constants see [their headers][pkcs11-headers].
//!
//! [pkcs11-v3]: https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.0/pkcs11-base-v3.0.html
//! [pkcs11-headers]: https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.0/cs01/include/pkcs11-v3.0/

use core::hint::unreachable_unchecked;
use crate::config;
use crate::types::*;

#[macro_use]
mod macros;

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Request {
    DummyRequest, // for testing
    CreateCounter(request::CreateCounter),
    FindObjects(request::FindObjects),
    GenerateKey(request::GenerateKey),
    GenerateKeypair(request::GenerateKeypair),
    ReadCounter(request::ReadCounter),
    Sign(request::Sign),
    Verify(request::Verify),
}

// TODO: Ideally, we would not need to assign random numbers here
// The only use for them is to check that the reply type corresponds
// to the request type in the client.

impl From<&Request> for u8 {
    fn from(request: &Request) -> u8 {
        match request {
            Request::DummyRequest => 0,
            Request::CreateCounter(_) => 1,
            Request::FindObjects(_) => 2,
            Request::GenerateKey(_) => 3,
            Request::GenerateKeypair(_) => 4,
            Request::ReadCounter(_) => 5,
            Request::Sign(_) => 6,
            Request::Verify(_) => 7,
        }
    }
}

impl From<&Reply> for u8 {
    fn from(reply: &Reply) -> u8 {
        match reply {
            Reply::DummyReply => 0,
            Reply::CreateCounter(_) => 1,
            Reply::FindObjects(_) => 2,
            Reply::GenerateKey(_) => 3,
            Reply::GenerateKeypair(_) => 4,
            Reply::ReadCounter(_) => 5,
            Reply::Sign(_) => 6,
            Reply::Verify(_) => 7,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Reply {
    DummyReply, // for testing
    CreateCounter(reply::CreateCounter),
    FindObjects(reply::FindObjects),
    GenerateKey(reply::GenerateKey),
    GenerateKeypair(reply::GenerateKeypair),
    ReadCounter(reply::ReadCounter),
    Sign(reply::Sign),
    Verify(reply::Verify),
}

pub mod request {
    use super::*;

    impl_request! {
        CreateCounter:
            // - attributes: Attributes,

        FindObjects:
            // - attributes: Attributes,

        GenerateKey:
            - mechanism: Mechanism
            - attributes: KeyAttributes

        GenerateKeypair:
            - mechanism: Mechanism
            - attributes: KeyAttributes
            // private_key_template: PrivateKeyTemplate
            // public_key_template: PublicKeyTemplate

        ReadCounter:
            - counter: ObjectHandle

        Sign:
          - mechanism: Mechanism
          - private_key: ObjectHandle
          - message: Message

        Verify:
          - mechanism: Mechanism
          - public_key: ObjectHandle
          - message: Message
          - signature: Signature
    }
}

pub mod reply {
    use super::*;

    // type ObjectHandles = Vec<ObjectHandle, config::MAX_OBJECT_HANDLES>;

    impl_reply! {
        CreateCounter:
            - counter: ObjectHandle

        FindObjects:
            - object_handles: Vec<ObjectHandle, config::MAX_OBJECT_HANDLES>
            // can be higher than capacity of vector
            - num_objects: usize

        GenerateKey:
            - secret_key: ObjectHandle

        GenerateKeypair:
            - private_key: ObjectHandle
            - public_key: ObjectHandle

        ReadCounter:
            - counter: u32

        Sign:
            - signature: Signature

        Verify:
            - valid: bool

    }

}

