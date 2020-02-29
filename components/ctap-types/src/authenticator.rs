//! The FIDO CTAP Authenticator API is a completely irregular RPC protocol.
//! Anytime there is some consistency in one place, another choice is made
//! in another place. Sorry!

pub enum Request {
    Ctap1(ctap1::Request),
    Ctap2(ctap2::Request),
}

pub enum Response {
    Ctap1(ctap1::Response),
    Ctap2(ctap2::Response),
}

pub mod ctap1 {
    pub enum Request {}
    pub enum Response {}

}
pub mod ctap2 {
    use crate::ctap2::*;

    pub enum Request {
        GetInfo,
        MakeCredential(make_credential::Parameters),
        GetAssertion(get_assertion::Parameters),
        GetNextAssertion,
        Reset,
    }

    pub enum Response {
        GetInfo(get_info::Response),
        MakeCredential(make_credential::Response),
        GetAssertion(get_assertion::Response),
        GetNextAssertion(get_assertion::Response),
        Reset,
    }

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
