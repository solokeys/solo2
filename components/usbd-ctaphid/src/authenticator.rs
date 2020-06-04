//! The idea here is to model the mandatory
//! and optional parts of the Authenticator API
//! as traits.
//!
//! The `usbd-ctaphid` layer is then supposed to handle
//! all kinds of low-level protocol details, leaving it
//! to the fido2 device to implement the actual functionality,
//! using nicer objects instead of transport-level bytes.
//!
//! TODO: Confirm that dependency injection of device logic
//! into CTAPHID driver is the right approach.

use crate::types::{
    AssertionResponses,
    AttestationObject,
    AuthenticatorInfo,
    GetAssertionParameters,
    MakeCredentialParameters,
};

// trait SimpleFuture {
//     type Output;
//     fn poll(&mut self, wake: fn()) -> Poll<Self::Output>;
// }

pub enum Ctap2Request {
    GetInfo,
    MakeCredential(MakeCredentialParameters),
    GetAssertions(GetAssertionParameters),
    Reset,
}

// hmm how to tie reponse type to request type
pub enum Ctap2Response {
    GetInfo(AuthenticatorInfo),
    MakeCredential(AttestationObject),
    GetAssertions(AssertionResponses),
    Reset,
}

pub trait Ctap2Api {

    fn process(&mut self, request: &mut Ctap2Request) -> Result<Ctap2Response>;

}

/// an authenticator implements this `authenticator::Api`.
/// TODO: modify interface so authenticator can process requests asynchronously.
/// Maybe with core::future::Future?
pub trait Api
{
    /// describe authenticator capabilities
    fn get_info(&mut self) -> AuthenticatorInfo;

    /// eventually generate a credential with specified options
    fn make_credential(&mut self, params: &MakeCredentialParameters)
        // TODO: use core::future::Future or something similar
        -> Result<AttestationObject>;

    fn get_assertions(&mut self, params: &GetAssertionParameters)
        -> Result<AssertionResponses>;

    fn reset(&mut self) -> Result<()>;
}

trait Wink {
    fn wink(&self);
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Error {
    Success = 0x00,
    InvalidCommand = 0x01,
    InvalidParameter = 0x02,
    InvalidLength = 0x03,
    InvalidSeq = 0x04,
    Timeout = 0x05,
    ChannelBusy = 0x06,
    LockRequired = 0x0A,
    InvalidChannel = 0x0B,
    CborUnexpectedType = 0x11,
    InvalidCbor = 0x12,
    MissingParameter = 0x14,
    LimitExceeded = 0x15,
    UnsupportedExtension = 0x16,
    CredentialExcluded = 0x19,
    Processing = 0x21,
    InvalidCredential = 0x22,
    UserActionPending = 0x23,
    OperationPending = 0x24,
    NoOperations = 0x25,
    UnsupportedAlgorithm = 0x26,
    OperationDenied = 0x27,
    KeyStoreFull = 0x28,
    NotBusy = 0x29,
    NoOperationPending = 0x2A,
    UnsupportedOption = 0x2B,
    InvalidOption = 0x2C,
    KeepaliveCancel = 0x2D,
    NoCredentials = 0x2E,
    UserActionTimeout = 0x2F,
    NotAllowed = 0x30,
    PinInvalid = 0x31,
    PinBlocked = 0x32,
    PinAuthInvalid = 0x33,
    PinAuthBlocked = 0x34,
    PinNotSet = 0x35,
    PinRequired = 0x36,
    PinPolicyViolation = 0x37,
    PinTokenExpired = 0x38,
    RequestTooLarge = 0x39,
    ActionTimeout = 0x3A,
    UpRequired = 0x3B,
    Other = 0x7F,
    SpecLast = 0xDF,
    ExtensionFirst = 0xE0,
    ExtensionLast = 0xEF,
    VendorFirst = 0xF0,
    VendorLast = 0xFF,
}

