//! This (incomplete!) API loosely follows PKCS#11 v3:
//! <https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.0/pkcs11-base-v3.0.html>

use crate::types::*;

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Request {
    DummyRequest,
    GenerateKey(GenerateKeyRequest),
    GenerateKeypair(GenerateKeypairRequest),
    Sign(SignRequest),
}

// pub struct DummyReply {}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Reply {
    DummyReply,
    GenerateKey(GenerateKeyReply),
    GenerateKeypair(GenerateKeypairReply),
    Sign(SignReply),
}

//
// GenerateKey
//

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GenerateKeyRequest {
    pub mechanism: Mechanism,
    pub key_parameters: KeyParameters,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GenerateKeyReply {
    pub key_handle: KeyHandle,
}

//
// GenerateKeypair
//

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GenerateKeypairRequest {
    pub mechanism: Mechanism,
    pub key_parameters: KeyParameters,
    // private_key_template: PrivateKeyTemplate,
    // public_key_template: PublicKeyTemplate,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GenerateKeypairReply {
    pub public_key_handle: KeyHandle,
    pub private_key_handle: KeyHandle,
}

//
// Sign
//

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SignRequest {
    pub key_handle: KeyHandle,
    pub mechanism: Mechanism,
    pub message: Message,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SignReply {
    pub signature: Signature,
}

