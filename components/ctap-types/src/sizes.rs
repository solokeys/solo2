use crate::consts;

#[allow(non_camel_case_types)]
pub type ATTESTED_CREDENTIAL_DATA_LENGTH = consts::U512;
// // not sure why i can't use `::to_usize()` here?
// pub const ATTESTED_CREDENTIAL_DATA_LENGTH_BYTES: usize = 512;

#[allow(non_camel_case_types)]
pub type AUTHENTICATOR_DATA_LENGTH = consts::U512;
// pub const AUTHENTICATOR_DATA_LENGTH_BYTES: usize = 512;

#[allow(non_camel_case_types)]
pub type ASN1_SIGNATURE_LENGTH = consts::U72;
// pub const ASN1_SIGNATURE_LENGTH_BYTES: usize = 72;

// #[allow(non_camel_case_types)]
// pub type COSE_KEY_LENGTH = consts::U256;
// pub const COSE_KEY_LENGTH_BYTES: usize = 256;
