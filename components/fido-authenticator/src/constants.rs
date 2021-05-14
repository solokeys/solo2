use trussed::types::{CertId, KeyId};

pub const FIDO2_UP_TIMEOUT: u32 = 30_000;
pub const U2F_UP_TIMEOUT: u32 =    0_250;

pub const ATTESTATION_CERT_ID: CertId = CertId::from_special(0);
pub const ATTESTATION_KEY_ID: KeyId = KeyId::from_special(0);
