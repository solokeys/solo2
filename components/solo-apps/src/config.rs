//! Shared FIDO authenticator config.
//!
//! `nfc_transport` MUST stay `true`: NFC-on-USB works, so the authenticator must
//! advertise NFC in `getInfo`'s `transports`, or platforms record credentials as
//! non-NFC and won't offer NFC for `getAssertion` (symptom: makeCredential works
//! over NFC but getAssertion doesn't). The const guard below keeps it that way.

/// FIDO authenticator config. `firmware_version` is the packed CTAP 2.1 §6.4 u32
/// `(major << 22) | (minor << 6) | patch`; `max_resident` is the resident-key cap
/// (lpc55 100, nrf52840dk 50).
pub const fn fido_config(firmware_version: u32, max_resident: u32) -> fido_authenticator::Config {
    fido_authenticator::Config {
        max_msg_size: ctaphid_dispatch::DEFAULT_MESSAGE_SIZE,
        skip_up_timeout: None,
        max_resident_credential_count: Some(max_resident),
        // CTAP 2.1 §6.10: minimum array size is 1024.
        large_blobs: Some(fido_authenticator::LargeBlobsConfig {
            location: trussed::types::Location::External,
            max_size: 1024,
        }),
        nfc_transport: true,
        ccid_transport: false,
        // Struct literal, not `.into()` (not const-callable here).
        firmware_version: Some(fido_authenticator::FirmwareVersion {
            default: firmware_version as usize,
            credential_id_v1: None,
            credential_id_v2: None,
        }),
        // V2 credential-id format: AES-256-GCM. Applied on a clean state / after
        // factory reset; existing V1 credentials persist.
        credential_id_version: Some(fido_authenticator::credential::CredentialIdVersion::V2),
        long_touch_for_reset: true,
        fido2_up_timeout: None,
    }
}

// Regression guard — the build fails if NFC advertisement is ever dropped.
const _: () = assert!(
    fido_config(0, 1).nfc_transport,
    "FIDO must advertise NFC in getInfo, else getAssertion won't use NFC on phones",
);
