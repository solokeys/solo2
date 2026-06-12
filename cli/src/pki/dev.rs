//! Overly simplistic "PKI" to enable all functionality of Solo 2 apps.
//!
//! In particular, there is no root CA, no use of hardware keys / PKCS #11,
//! no revocation, etc. etc.

use rand_core::{OsRng, RngCore};

#[repr(u16)]
pub enum Kind {
    Shared = 1,
    Symmetric = 2,
    Symmetric32Nonce = 3,
    Ed255 = 4,
    P256 = 5,
    X255 = 6,
}

fn trussed_serialized_key(sensitive: bool, kind: Kind, material: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut flags = 0u16;
    // we shouldn't be in the business of injecting "local" keys
    // if local { flags |= 1; }
    if sensitive {
        flags |= 2;
    }

    buffer.extend_from_slice(flags.to_be_bytes().as_ref());
    buffer.extend_from_slice((kind as u16).to_be_bytes().as_ref());
    buffer.extend_from_slice(material);

    buffer
}

pub fn generate_selfsigned_fido() -> ([u8; 16], [u8; 36], String, rcgen::Certificate) {
    let alg = &rcgen::PKCS_ECDSA_P256_SHA256;

    // 0. generate AAGUID
    let mut aaguid = [0u8; 16];
    OsRng.fill_bytes(&mut aaguid);

    // 1. generate a keypair, massage into Trussed keystore binary format
    let keypair = rcgen::KeyPair::generate(alg).unwrap();

    let key_pkcs8 = keypair.serialize_der();
    let key_pem = keypair.serialize_pem();

    use p256::pkcs8::PrivateKeyInfo;
    let key_info: PrivateKeyInfo = key_pkcs8.as_slice().try_into().unwrap();
    let secret_key: [u8; 32] = p256::SecretKey::try_from(key_info)
        .unwrap()
        .to_bytes()
        .into();

    let sensitive = true;
    let kind = Kind::P256;
    let key_trussed: [u8; 36] = trussed_serialized_key(sensitive, kind, secret_key.as_ref())
        .try_into()
        .unwrap();

    // 2. generate self-signed certificate

    let mut tbs = rcgen::CertificateParams::default();
    tbs.alg = alg;
    tbs.serial_number = Some(OsRng.next_u64());

    let now = time::OffsetDateTime::now_utc();
    tbs.not_before = now;
    tbs.not_after = now + time::Duration::days(50 * 365);

    // https://www.w3.org/TR/webauthn-2/#sctn-packed-attestation-cert-requirements
    let mut subject = rcgen::DistinguishedName::new();
    subject.push(rcgen::DnType::CountryName, "AQ");
    subject.push(rcgen::DnType::OrganizationName, "Example Vendor");
    subject.push(
        rcgen::DnType::OrganizationalUnitName,
        "Authenticator Attestation",
    );
    subject.push(rcgen::DnType::CommonName, "example.com");
    tbs.distinguished_name = subject;

    tbs.key_pair = Some(keypair);
    tbs.is_ca = rcgen::IsCa::NoCa;
    // TODO: check if `authorityKeyIdentifier=keyid,issuer` is both needed
    // NB: for self-signed, rcgen does not follow this instruction
    tbs.use_authority_key_identifier_extension = true;

    let extensions = vec![
        // AAGUID
        // https://www.w3.org/TR/webauthn-2/#sctn-packed-attestation-cert-requirements
        // id-fido-gen-ce-aaguid
        // Not technically necessary, as we don't have "multiple models", just a new random key + cert.
        rcgen::CustomExtension::from_oid_content(
            // FIDO's PEN + 1.1.4
            &[1, 3, 6, 1, 4, 1, 45724, 1, 1, 4],
            yasna::construct_der(|writer| writer.write_bytes(aaguid.as_ref())),
        ),
    ];
    // cf. the following, this is not necessary
    // https://groups.google.com/a/fidoalliance.org/g/fido-dev/c/pfuWJvM-OQQ
    // https://github.com/w3c/webauthn/issues/817
    // // Transports
    // extensions.push(rcgen::CustomExtension::from_oid_content(
    //     // FIDO's PEN + 2.1.1
    //     &[1, 3, 6, 1, 4, 1, 45724, 2, 1, 1],
    //     // https://fidoalliance.org/fido-technote-detailed-look-fido-u2f-v1-2/#:~:text=transports-,FIDOU2FTransports%20%3A%3A%3D,uSBInternal(4,-Raw
    //     {
    //         let mut bits = 0u8;
    //         // USB
    //         bits |= 1 << 2;
    //         if nfc {
    //             bits |= 1 << 3;
    //         }
    //         yasna::construct_der(|writer| writer.write_bitvec_bytes(&[bits], 4))
    //     },
    // ));
    tbs.custom_extensions = extensions;

    let cert = rcgen::Certificate::from_params(tbs).unwrap();

    (aaguid, key_trussed, key_pem, cert)
}
