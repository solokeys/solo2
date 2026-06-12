//! # Solo 2 PKI
//!
//! Each Solo 2 device has a unique identity, its 128 bit UUID, which is set by NXP in read-only
//! memory.  Backing this up is a PKI infrastructure, hosted under <https://s2pki.net/>, and
//! explained in the following.
//!
//! At the root is [`R1`][r1], an offline RSA-4096 keypair with a self-signed certificate.
//! It can be obtained via <http://i.s2pki.net/r1/>.
//!
//! ## Trussed certificates
//!
//! Under `R1` sit the intermediate Trussed certificate authorities [`T1`][t1] and [`T2`][t2], which have
//! an Ed255 and P256 keypair, respectively. They are signed by `R1` with pathlen = 1.
//! The certificates are available via:
//! - <https://i.s2pki.net/t1/>
//! - <https://i.s2pki.net/t2/>
//!
//! We have two since there is use for both NIST-based (P256) and djb-based (Ed255/X255) certificate chains.
//!
//! Each Solo 2 device then has three embedded certificates, backed by three keypairs which are
//! generated on-device during production, after the device has been locked. In their X509v3
//! extension with OID `1.3.6.1.4.1.54053.1.1` they contain the UUID of the device.
//! The certificates are as follows:
//!
//! - The Ed255 Trussed device leaf certificate (pathlen = 0), signed by `T1`, with key usages
//!   `Certificate Sign` and `CRL Sign`.
//! - The X255 Trussed device entity certificate, signed by `T1`, with key usage `Key Agreement`
//! - the P256 Trussed device leaf certificate (pathlen = 0), signed by `T2`, with key usages
//!   `Certificate Sign`, `CRL Sign`, and `Key Agreement`.
//!
//! ## Firmware certificates
//!
//! The NXP bootloader's secure boot mechanism has space for four certificates, which may be revoked
//! individually. Correspondingly, we have four entity certificates `S1`, `S2`, `S3`, `S4`, split
//! designated as active/backup and US/CH development centers. They are available from
//! <http://i.s2pki.net/s1/>, etc. For firmware signing purposes, they are self-signed; from a PKI
//! perspective we additionally cross-certified them via `R1`.
//!
//! ## FIDO certificates
//!
//! There is an intermediate CA called `F1`, which signs the batch certificates for FIDO metadata,
//! which are used during device attestation. These batch certificates must be model specific, we
//! have prepared one each for Solo 2A+ (USB-A + NFC), Solo 2C+ (USB-C + NFC), Solo 2A (USB-A
//! only), Solo 2C (USB-C only).
//!
//! [r1]: https://s2pki.net/i/r1/r1.txt
//! [t1]: https://s2pki.net/i/t1/t1.txt
//! [t2]: https://s2pki.net/i/t2/t2.txt

#[cfg(feature = "dev-pki")]
pub mod dev;

use std::io::Read as _;

pub use x509_parser::certificate::X509Certificate;

use crate::Result;

pub const S2PKI_TLD: &str = "s2pki.net";

/// Certificate authorities for Solo 2 PKI.
///
/// For more information, read [pki][crate::pki] module level documentation.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Authority {
    /// The root RSA-4096 certificate authority
    R1,
    /// The Trussed intermediate CA for Ed255 + X255
    T1,
    /// The Trussed intermediate CA for P256
    T2,
    /// Solo 2 firmware signing authority US active
    S1,
    /// Solo 2 firmware signing authority US backup
    S2,
    /// Solo 2 firmware signing authority CH active
    S3,
    /// Solo 2 firmware signing authority CH backup
    S4,
    /// The SoloKeys FIDO intermediate CA (P256)
    F1,
    /// Current Solo 2A+ FIDO batch entity certificate
    B1,
    /// Current Solo 2C+ FIDO batch entity certificate
    B2,
    /// Current Solo 2A FIDO batch entity certificate
    B3,
    /// Current Solo 2C FIDO batch entity certificate
    B4,
}

impl Authority {
    pub fn name(&self) -> String {
        format!("{:?}", self)
    }
}

impl TryFrom<&str> for Authority {
    type Error = crate::Error;
    fn try_from(name: &str) -> Result<Authority> {
        Ok(match name.to_uppercase().as_str() {
            "B1" => Authority::B1,
            "B2" => Authority::B2,
            "B3" => Authority::B3,
            "B4" => Authority::B4,
            "F1" => Authority::F1,
            "R1" => Authority::R1,
            "S1" => Authority::S1,
            "S2" => Authority::S2,
            "S3" => Authority::S3,
            "S4" => Authority::S4,
            "T1" => Authority::T1,
            "T2" => Authority::T2,
            _ => return Err(anyhow::anyhow!("Unknown authority name {}", name)),
        })
    }
}

/// An owned wrapper for `x509_parser::certificate::X509Certificate`.
///
/// In `lpc55`, we enforce RSA signatures...
#[derive(Clone, Debug)]
pub struct Certificate {
    der: Vec<u8>,
}

impl Certificate {
    pub fn try_from_der(der: &[u8]) -> Result<Self> {
        use x509_parser::prelude::FromDer;
        X509Certificate::from_der(der)?;
        Ok(Self { der: der.to_vec() })
    }

    pub fn der(&self) -> &[u8] {
        &self.der
    }

    pub fn certificate(&self) -> X509Certificate<'_> {
        use x509_parser::prelude::FromDer;
        X509Certificate::from_der(&self.der).unwrap().1
    }
}

/// Canonical URI for Authority Information Access (i.e., where to get the certificate in DER
/// format).
///
/// This is `http://i.s2pki.net/{lower case CA name}/`.
///
/// There are also other formats available, e.g., `https://s2pki.net/i/r1/r1.{der,pem,txt}`.
pub fn authority_information_access(authority: Authority) -> String {
    format!("http://i.{}/{:?}/", S2PKI_TLD, authority).to_lowercase()
}

/// Download the certificate of an [`Authority`].
pub fn fetch_certificate(authority: Authority) -> Result<Certificate> {
    let mut der = Vec::new();
    ureq::get(&authority_information_access(authority))
        .call()?
        .into_reader()
        .read_to_end(&mut der)?;
    Certificate::try_from_der(&der)
}

#[cfg(all(test, feature = "network-tests"))]
mod test {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(
            authority_information_access(Authority::R1),
            "http://i.s2pki.net/r1/"
        );
    }

    #[test]
    fn r1() {
        let r1 = fetch_certificate(Authority::R1).unwrap();
        assert_eq!(r1.der(), include_bytes!("../data/r1.der"),);
    }

    #[test]
    fn t1t2() {
        let r1 = fetch_certificate(Authority::R1).unwrap();
        // lifetimes...
        let r1 = r1.certificate();
        let r1_pubkey = Some(r1.public_key());
        let t1 = fetch_certificate(Authority::T1).unwrap();
        let t2 = fetch_certificate(Authority::T2).unwrap();
        assert!(t1.certificate().verify_signature(r1_pubkey).is_ok());
        assert!(t2.certificate().verify_signature(r1_pubkey).is_ok());
    }
}
