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

// TODO: Ideally, we would not need to assign random numbers here
// The only use for them is to check that the reply type corresponds
// to the request type in the client.
//
// At minimum, we don't want to list the indices (may need proc-macro)

generate_enums! {
    CreateObject: 1
    DeriveKey: 2
    // DeriveKeypair: 3
    FindObjects: 4
    GenerateKey: 5
    // GenerateKeypair: 6
    // ReadCounter: 7
    Sign: 8
    UnwrapKey: 9
    Verify: 10
    WrapKey: 11
}

pub mod request {
    use super::*;

    impl_request! {
        // examples:
        // - store public keys from external source
        // - store certificates
        CreateObject:
            - attributes: Attributes

        // examples:
        // - public key from private key
        // - Diffie-Hellman
        // - hierarchical deterministic wallet stuff
        DeriveKey:
            - mechanism: Mechanism
            - base_key: ObjectHandle
            // - auxiliary_key: Option<ObjectHandle>
            // - additional_data: LongData
            // - attributes: KeyAttributes

        // DeriveKeypair:
        //     - mechanism: Mechanism
        //     - base_key: ObjectHandle
        //     // - additional_data: Message
        //     // - attributes: KeyAttributes

        FindObjects:
            // - attributes: Attributes,

        GenerateKey:
            - mechanism: Mechanism        // -> implies key type
            - attributes: KeyAttributes

        // use GenerateKey + DeriveKey(public-from-private) instead
        // GenerateKeypair:
        //     - mechanism: Mechanism
        //     - attributes: KeyAttributes
        //     // private_key_template: PrivateKeyTemplate
        //     // public_key_template: PublicKeyTemplate

        // GetAttributes:
        //     - object: ObjectHandle
        //     - attributes: Attributes

        // use GetAttribute(value) on counter instead
        // ReadCounter:
        //     - counter: ObjectHandle

        Sign:
          - mechanism: Mechanism
          - key: ObjectHandle
          - message: Message

        UnwrapKey:
          - mechanism: Mechanism
          - wrapping_key: ObjectHandle
          - wrapped_key: Message
          - associated_data: Message

        Verify:
          - mechanism: Mechanism
          - key: ObjectHandle
          - message: Message
          - signature: Signature

        // this should always be an AEAD algorithm
        WrapKey:
          - mechanism: Mechanism
          - wrapping_key: ObjectHandle
          - key: ObjectHandle
          - associated_data: Message

    }
}

pub mod reply {
    use super::*;

    // type ObjectHandles = Vec<ObjectHandle, config::MAX_OBJECT_HANDLES>;

    impl_reply! {
        CreateObject:
            - object: ObjectHandle

        FindObjects:
            - objects: Vec<ObjectHandle, config::MAX_OBJECT_HANDLES>
            // can be higher than capacity of vector
            - num_objects: usize

        DeriveKey:
            - key: ObjectHandle

        // DeriveKeypair:
        //     - private_key: ObjectHandle
        //     - public_key: ObjectHandle

        GenerateKey:
            - key: ObjectHandle

        // GenerateKeypair:
        //     - private_key: ObjectHandle
        //     - public_key: ObjectHandle

        // ReadCounter:
        //     - counter: u32

        Sign:
            - signature: Signature

        Verify:
            - valid: bool

        UnwrapKey:
            - key: Option<ObjectHandle>

        WrapKey:
            - wrapped_key: Message

    }

}

// TODO: can we find a nicer syntax for this?
generate_api! {
    CreateObject: 1
    in: {
        attributes: Attributes
    }
    out: {
        object: ObjectHandle
    }

    DeriveKey: 2
    in: {
        mechanism: Mechanism
        base_key: ObjectHandle
    }
    out: {
        derived_key: ObjectHandle
    }

}

