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
    Agree: 12
    CreateObject: 1
    Decrypt: 13
    DeriveKey: 2
    DeserializeKey: 17
    Encrypt: 14
    // DeriveKeypair: 3
    FindObjects: 4
    GenerateKey: 5
    // GenerateKeypair: 6
    LoadBlob: 15
    // ReadCounter: 7
    SerializeKey: 16
    Sign: 8
    StoreBlob: 16
    UnwrapKey: 9
    Verify: 10
    WrapKey: 11
}

pub mod request {
    use super::*;

    impl_request! {
        Agree:
            - mechanism: Mechanism
            - private_key: ObjectHandle
            - public_key: ObjectHandle
            - attributes: StorageAttributes

        // examples:
        // - store public keys from external source
        // - store certificates
        CreateObject:
            - attributes: Attributes

        Decrypt:
          - mechanism: Mechanism
          - key: ObjectHandle
          - message: Message
          - associated_data: Message
          - nonce: ShortData
          - tag: ShortData

        DeserializeKey:
          - mechanism: Mechanism
          - serialized_key: Message

        // DeleteBlob:
        //   - prefix: Option<Letters>
        //   - name: ShortData

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
            - attributes: StorageAttributes

        // DeriveKeypair:
        //     - mechanism: Mechanism
        //     - base_key: ObjectHandle
        //     // - additional_data: Message
        //     // - attributes: KeyAttributes

        Encrypt:
          - mechanism: Mechanism
          - key: ObjectHandle
          - message: Message
          - associated_data: ShortData

        FindObjects:
            // - attributes: Attributes,

        GenerateKey:
            - mechanism: Mechanism        // -> implies key type
            // - attributes: KeyAttributes
            - attributes: StorageAttributes

        // use GenerateKey + DeriveKey(public-from-private) instead
        // GenerateKeypair:
        //     - mechanism: Mechanism
        //     - attributes: KeyAttributes
        //     // private_key_template: PrivateKeyTemplate
        //     // public_key_template: PublicKeyTemplate

        // GetAttributes:
        //     - object: ObjectHandle
        //     - attributes: Attributes

        LoadBlob:
          - prefix: Option<Letters>
          - id: ObjectHandle
          // - id: MediumData
          - location: StorageLocation

        // use GetAttribute(value) on counter instead
        // ReadCounter:
        //     - counter: ObjectHandle

        SerializeKey:
          - mechanism: Mechanism
          - key: ObjectHandle
          - format: KeySerialization

        Sign:
          - mechanism: Mechanism
          - key: ObjectHandle
          - message: Message

        StoreBlob:
          - prefix: Option<Letters>
          // - id: MediumData
          - data: Message
          - attributes: StorageAttributes

        UnwrapKey:
          - mechanism: Mechanism
          - wrapping_key: ObjectHandle
          - wrapped_key: Message
          - associated_data: Message
          - attributes: StorageAttributes

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
        // could return either a SharedSecretXY or a SymmetricKeyXY,
        // depending on mechanism
        // e.g.: P256Raw -> SharedSecret32
        //       P256Sha256 -> SymmetricKey32
        Agree:
            - shared_secret: ObjectHandle

        CreateObject:
            - object: ObjectHandle

        FindObjects:
            - objects: Vec<ObjectHandle, config::MAX_OBJECT_HANDLES>
            // can be higher than capacity of vector
            - num_objects: usize

		Decrypt:
            - plaintext: Message

        DeriveKey:
            - key: ObjectHandle

        DeserializeKey:
            - key: ObjectHandle

		Encrypt:
            - ciphertext: Message
            - nonce: ShortData
            - tag: ShortData

        // DeriveKeypair:
        //     - private_key: ObjectHandle
        //     - public_key: ObjectHandle

        GenerateKey:
            - key: ObjectHandle

        // GenerateKeypair:
        //     - private_key: ObjectHandle
        //     - public_key: ObjectHandle

        LoadBlob:
          - data: Message

        // ReadCounter:
        //     - counter: u32

        SerializeKey:
            - wrapped_key: Message

        Sign:
            - signature: Signature

        StoreBlob:
            - blob: ObjectHandle

        Verify:
            - valid: bool

        UnwrapKey:
            - key: ObjectHandle

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

