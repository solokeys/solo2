//! This (incomplete!) API loosely follows [PKCS#11 v3][pkcs11-v3].
//!
//! For constants see [their headers][pkcs11-headers].
//!
//! [pkcs11-v3]: https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.0/pkcs11-base-v3.0.html
//! [pkcs11-headers]: https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.0/cs01/include/pkcs11-v3.0/

use crate::config;
use crate::types::*;

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Request {
    DummyRequest, // for testing
    CreateCounter(request::CreateCounter),
    FindObjects(request::FindObjects),
    GenerateKey(request::GenerateKey),
    GenerateKeypair(request::GenerateKeypair),
    ReadCounter(request::ReadCounter),
    Sign(request::Sign),
}

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
}

impl From<Reply> for reply::CreateCounter {
    fn from(reply: Reply) -> reply::CreateCounter {
        match reply {
            Reply::CreateCounter(reply) => reply,
            _ => { unsafe { unreachable!() } }
        }
    }
}

impl From<Reply> for reply::ReadCounter {
    fn from(reply: Reply) -> reply::ReadCounter {
        match reply {
            Reply::ReadCounter(reply) => reply,
            _ => { unsafe { unreachable!() } }
        }
    }
}

impl From<Reply> for reply::GenerateKeypair {
    fn from(reply: Reply) -> reply::GenerateKeypair {
        match reply {
            Reply::GenerateKeypair(reply) => reply,
            _ => { unsafe { unreachable!() } }
        }
    }
}

impl From<Reply> for reply::Sign {
    fn from(reply: Reply) -> reply::Sign {
        match reply {
            Reply::Sign(reply) => reply,
            _ => { unsafe { unreachable!() } }
        }
    }
}

// macro_rules! impl_ {
// }

// impl_! {
//     CreateCounter:
//         (
//             attributes: Attributes,
//         ) -> (
//             object_handles: Vec<ObjectHandle, config::MAX_OBJECT_HANDLES>,
//         )

// }

pub mod request {
    use super::*;

    // monotonically increasing counter
    // no reset - if you need that, delete
    // the counter and create a new one
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct CreateCounter {
        // pub attributes: Attributes,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct FindObjects {
        // pub attributes: Attributes,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct GenerateKey {
        pub mechanism: Mechanism,
        pub key_attributes: KeyAttributes,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct GenerateKeypair {
        pub mechanism: Mechanism,
        pub key_attributes: KeyAttributes,
        // private_key_template: PrivateKeyTemplate,
        // public_key_template: PublicKeyTemplate,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct ReadCounter {
        pub counter_handle: ObjectHandle,
    }

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct Sign {
        pub key_handle: ObjectHandle,
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

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct CreateCounter {
        pub key_handle: ObjectHandle,
    }

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct FindObjects {
        pub object_handles: Vec<ObjectHandle, config::MAX_OBJECT_HANDLES>,
        // can be higher than capacity of vector
        pub num_objects: usize,
    }

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct GenerateKey {
        pub key_handle: ObjectHandle,
    }

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct GenerateKeypair {
        pub keypair_handle: ObjectHandle,
        // pub public_key_handle: ObjectHandle,
        // pub private_key_handle: ObjectHandle,
    }

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct ReadCounter {
        pub counter: u32,
    }

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct Sign {
        pub signature: Signature,
    }

}
