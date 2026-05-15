//! largeBlobKey extension tests (CTAP 2.1 §11.4).
//!
//! Ported from the FIDO conformance module
//! `tests/CTAP2/Protocol/Extensions/largeBlobKey.js`.
//!
//! The largeBlobKey extension returns a per-credential 32-byte key in the
//! MakeCredential (0x05) / GetAssertion (0x05) *top-level* response (NOT inside
//! authData.extensions). It is only meaningful for resident (discoverable)
//! credentials.
//!
//! DEVICE-ONLY: the `with_authenticator!` sim Config hardcodes
//! `large_blobs: None`, so the device never advertises `largeBlobKey` and never
//! produces the key in sim. Every test here gates on `is_device_mode()` and
//! trivially passes in sim/CI; it actually exercises on hardware
//! (`FIDO2_TRANSPORT=device`).

use super::*;
use support::raw;

/// MakeCredential request CBOR with a resident key and an explicit `extensions`
/// map (key 0x06). `lbk` becomes the value of `largeBlobKey` in that map.
/// `rp_id`/`user_id` keep credentials distinct across calls.
fn mc_cbor_with_large_blob_key(
    rp_id: &str,
    user_id: &[u8],
    user_name: &str,
    lbk: Value,
) -> Vec<u8> {
    use ctap_types::webauthn::*;

    let mut params = FilteredPublicKeyCredentialParameters(heapless::Vec::new());
    params
        .0
        .push(KnownPublicKeyCredentialParameters { alg: -7 })
        .ok();
    let rp = PublicKeyCredentialRpEntity {
        id: rp_id.try_into().unwrap(),
        name: Some("Example".try_into().unwrap()),
        icon: None,
    };
    let user = PublicKeyCredentialUserEntity {
        id: ctap_types::Bytes::try_from(user_id).unwrap(),
        icon: None,
        name: Some(user_name.try_into().unwrap()),
        display_name: Some("Test User".try_into().unwrap()),
    };

    let extensions = raw::map([(raw::text("largeBlobKey"), lbk)]);
    let value = raw::map([
        (raw::int_key(1), raw::bytes([0xcd_u8; 32])),
        (raw::int_key(2), to_value(&rp).expect("serialize rp")),
        (raw::int_key(3), to_value(&user).expect("serialize user")),
        (
            raw::int_key(4),
            to_value(&params).expect("serialize params"),
        ),
        (raw::int_key(6), extensions),
        (raw::int_key(7), options_value(Some(true), None, None)),
    ]);
    raw::encode(&value)
}

/// GetAssertion request CBOR with an allow-list of one and an explicit
/// `extensions` map (key 0x04) whose `largeBlobKey` is `lbk`.
fn ga_cbor_with_large_blob_key(rp_id: &str, credential_id: &[u8], lbk: Value) -> Vec<u8> {
    let allow_entry = raw::map([
        (raw::text("type"), raw::text("public-key")),
        (raw::text("id"), raw::bytes_vec(credential_id.to_vec())),
    ]);
    let extensions = raw::map([(raw::text("largeBlobKey"), lbk)]);
    let value = raw::map([
        (raw::int_key(1), raw::text(rp_id)),
        (raw::int_key(2), raw::bytes([0xcd_u8; 32])),
        (raw::int_key(3), raw::array([allow_entry])),
        (raw::int_key(4), extensions),
    ]);
    raw::encode(&value)
}

const RP_ID: &str = "largeblobkey.example.com";

/// P-1: MakeCredential with `largeBlobKey=true` returns a 32-byte largeBlobKey
/// (0x05); a second credential gets a fresh, different key.
///
/// P-2: GetAssertion with `largeBlobKey=true` for that credential returns the
/// same 32-byte key recorded at registration.
#[test]
#[serial]
fn large_blob_key_make_credential_and_get_assertion() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(large_blob_key_mc_ga, |authn| {
            reset_authenticator(authn);

            // --- P-1: first credential ---
            let mut req = make_credential_request_for(RP_ID, &[0x01; 16], "lbk-user-1", true);
            let mut ext = ctap2::make_credential::ExtensionsInput::default();
            ext.large_blob_key = Some(true);
            req.extensions = Some(ext);
            up::approve();
            let (cred_id, key1) = match authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("MC with largeBlobKey=true should succeed")
            {
                Response::MakeCredential(mc) => {
                    let key = mc
                        .large_blob_key
                        .expect("MakeCredential response missing largeBlobKey (0x05)");
                    assert_eq!(key.len(), 32, "largeBlobKey must be exactly 32 bytes");
                    (extract_credential_id(&mc.auth_data), *key)
                }
                other => panic!("Expected MakeCredential, got {:?}", other),
            };

            // --- P-1 cont.: a fresh credential must get a different key ---
            let mut req2 = make_credential_request_for(RP_ID, &[0x02; 16], "lbk-user-2", true);
            let mut ext2 = ctap2::make_credential::ExtensionsInput::default();
            ext2.large_blob_key = Some(true);
            req2.extensions = Some(ext2);
            up::approve();
            match authn
                .call_ctap2(&Request::MakeCredential(req2))
                .expect("second MC with largeBlobKey=true should succeed")
            {
                Response::MakeCredential(mc) => {
                    let key2 = mc
                        .large_blob_key
                        .expect("second MakeCredential response missing largeBlobKey (0x05)");
                    assert_eq!(key2.len(), 32, "largeBlobKey must be exactly 32 bytes");
                    assert_ne!(
                        *key2, key1,
                        "each credential must get a fresh, distinct largeBlobKey"
                    );
                }
                other => panic!("Expected MakeCredential, got {:?}", other),
            }

            // --- P-2: GetAssertion returns the same key as P-1 ---
            let mut ga = get_assertion_request_for(RP_ID, Some(single_allow_list(&cred_id)));
            let mut ga_ext = ctap2::get_assertion::ExtensionsInput::default();
            ga_ext.large_blob_key = Some(true);
            ga.extensions = Some(ga_ext);
            up::approve();
            match authn
                .call_ctap2(&Request::GetAssertion(ga))
                .expect("GA with largeBlobKey=true should succeed")
            {
                Response::GetAssertion(ga) => {
                    let key = ga
                        .large_blob_key
                        .expect("GetAssertion response missing largeBlobKey (0x05)");
                    assert_eq!(key.len(), 32, "largeBlobKey must be exactly 32 bytes");
                    assert_eq!(
                        *key, key1,
                        "GetAssertion largeBlobKey must match the one from registration"
                    );
                }
                other => panic!("Expected GetAssertion, got {:?}", other),
            }
        })
    });
}

/// F-1: MakeCredential with `largeBlobKey=false` -> CTAP2_ERR_INVALID_OPTION (0x2C).
#[test]
#[serial]
fn large_blob_key_make_credential_false_is_invalid_option() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(large_blob_key_mc_false, |authn| {
            reset_authenticator(authn);

            let mut req = make_credential_request_for(RP_ID, &[0x11; 16], "lbk-false", true);
            let mut ext = ctap2::make_credential::ExtensionsInput::default();
            ext.large_blob_key = Some(false);
            req.extensions = Some(ext);
            up::approve();
            assert_eq!(
                authn.call_ctap2(&Request::MakeCredential(req)),
                Err(ctap2::Error::InvalidOption),
                "MC with largeBlobKey=false must return INVALID_OPTION (0x2C)"
            );
        })
    });
}

/// F-2: MakeCredential with `largeBlobKey` of non-boolean type ->
/// CTAP2_ERR_CBOR_UNEXPECTED_TYPE (0x11). Built as raw CBOR because the typed
/// builder only accepts `Option<bool>`.
#[test]
#[serial]
fn large_blob_key_make_credential_non_boolean_is_unexpected_type() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(large_blob_key_mc_type, |authn| {
            reset_authenticator(authn);

            // largeBlobKey as a text string instead of a boolean.
            let payload = mc_cbor_with_large_blob_key(
                RP_ID,
                &[0x12; 16],
                "lbk-type",
                raw::text("not-a-bool"),
            );
            up::approve();
            let (status, _body) = authn
                .call_ctap2_raw(0x01, &payload)
                .expect("raw MakeCredential transport should round-trip");
            assert_eq!(
                status, 0x11,
                "MC with non-boolean largeBlobKey must return CBOR_UNEXPECTED_TYPE (0x11)"
            );
        })
    });
}

/// F-3: GetAssertion with `largeBlobKey=false` -> CTAP2_ERR_INVALID_OPTION (0x2C).
#[test]
#[serial]
fn large_blob_key_get_assertion_false_is_invalid_option() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(large_blob_key_ga_false, |authn| {
            reset_authenticator(authn);

            // Register a credential to assert against.
            let mut req = make_credential_request_for(RP_ID, &[0x21; 16], "lbk-ga-false", true);
            let mut ext = ctap2::make_credential::ExtensionsInput::default();
            ext.large_blob_key = Some(true);
            req.extensions = Some(ext);
            up::approve();
            let cred_id = match authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("setup MC should succeed")
            {
                Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                other => panic!("Expected MakeCredential, got {:?}", other),
            };

            let mut ga = get_assertion_request_for(RP_ID, Some(single_allow_list(&cred_id)));
            let mut ga_ext = ctap2::get_assertion::ExtensionsInput::default();
            ga_ext.large_blob_key = Some(false);
            ga.extensions = Some(ga_ext);
            up::approve();
            assert_eq!(
                authn.call_ctap2(&Request::GetAssertion(ga)),
                Err(ctap2::Error::InvalidOption),
                "GA with largeBlobKey=false must return INVALID_OPTION (0x2C)"
            );
        })
    });
}

/// F-4: GetAssertion with `largeBlobKey` of non-boolean type ->
/// CTAP2_ERR_CBOR_UNEXPECTED_TYPE (0x11). Built as raw CBOR.
#[test]
#[serial]
fn large_blob_key_get_assertion_non_boolean_is_unexpected_type() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(large_blob_key_ga_type, |authn| {
            reset_authenticator(authn);

            // Register a credential to assert against.
            let mut req = make_credential_request_for(RP_ID, &[0x31; 16], "lbk-ga-type", true);
            let mut ext = ctap2::make_credential::ExtensionsInput::default();
            ext.large_blob_key = Some(true);
            req.extensions = Some(ext);
            up::approve();
            let cred_id = match authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("setup MC should succeed")
            {
                Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                other => panic!("Expected MakeCredential, got {:?}", other),
            };

            // largeBlobKey as an integer instead of a boolean.
            let payload = ga_cbor_with_large_blob_key(RP_ID, &cred_id, Value::Integer(42));
            up::approve();
            let (status, _body) = authn
                .call_ctap2_raw(0x02, &payload)
                .expect("raw GetAssertion transport should round-trip");
            assert_eq!(
                status, 0x11,
                "GA with non-boolean largeBlobKey must return CBOR_UNEXPECTED_TYPE (0x11)"
            );
        })
    });
}
