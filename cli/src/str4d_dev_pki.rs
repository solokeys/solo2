//! Overly simplistic "PKI" to enable all functionality of Solo 2 apps.
//!
//! In particular, there is no root CA, no use of hardware keys / PKCS #11,
//! no revocation, etc. etc.

use rand_core::{RngCore, OsRng};
use x509::RelativeDistinguishedName;

struct MockAlgorithmId;

impl AlgorithmIdentifier for MockAlgorithmId {
    type AlgorithmOid = &'static [u64];

    fn algorithm(&self) -> Self::AlgorithmOid {
        &[1, 1, 1, 1]
    }

    fn parameters<W: std::io::Write>(
        &self,
        w: cookie_factory::WriteContext<W>,
    ) -> cookie_factory::GenResult<W> {
        Ok(w)
    }
}

pub fn generate_fido(nfc: bool) {
    let mut serial = [0u8; 20];
    OsRng.fill_bytes(serial.as_mut());

    let algorithm =
    let issuer = [
        RelativeDistinguishedName::country("AQ"),
        RelativeDistinguishedName::organization("Fake Organization"),
        RelativeDistinguishedName::organizational_unit("Fake FIDO Attestation"),
        RelativeDistinguishedName::common_name("example.com"),
    ];

    let not_before = chrono::Utc::now();
    let not_after = None,

    let extensions = [];

    let tbs = x509::write::tbs_certificate(
        &serial,
        &issuer,
        &not_before,
        &not_after,
        &extensions;

    );

}
