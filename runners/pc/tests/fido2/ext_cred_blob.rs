//! credBlob extension tests.
//!
//! Ported from the FIDO CTAP2.3 conformance module
//! (`tests/CTAP2/Protocol/Extensions/credBlob.js`).
//!
//! The conformance suite obtains a pinUvAuthToken before each case. Our
//! device advertises `alwaysUv: false` and `makeCredUvNotRqd: true`, so
//! credBlob can be set and retrieved without a PIN. We therefore exercise
//! the extension over plain (user-presence-only) MakeCredential /
//! GetAssertion, matching the style of the sibling extension modules
//! (cred_protect.rs, hmac_secret.rs).

use super::*;

/// Decode the extensions CBOR map appended to a GetAssertion `auth_data`
/// (no attested credential data: 32-byte rpIdHash + 1 flags + 4 counter,
/// then the extensions map when the ED flag is set), returning the raw
/// `credBlob` byte string if present.
fn get_assertion_cred_blob(auth_data: &[u8]) -> Option<Vec<u8>> {
    // ED (extension data) flag is bit 7 of the flags byte at offset 32.
    if auth_data[32] & 0x80 == 0 {
        return None;
    }
    let ext_bytes = &auth_data[37..];
    let map: serde_cbor::Value = serde_cbor::from_slice(ext_bytes).expect("decode extensions map");
    match map {
        serde_cbor::Value::Map(entries) => {
            for (k, v) in entries {
                if k == serde_cbor::Value::Text("credBlob".to_string()) {
                    match v {
                        serde_cbor::Value::Bytes(b) => return Some(b),
                        _ => panic!("credBlob extension output must be a byte string"),
                    }
                }
            }
            None
        }
        other => panic!("extensions data is not a CBOR map: {other:?}"),
    }
}

/// Make a discoverable credential carrying a `credBlob` extension input.
fn mc_with_cred_blob(
    authn: &mut dyn TestAuthenticator,
    rp_id: &str,
    user_id: &[u8],
    blob: &[u8],
) -> Vec<u8> {
    let mut req = make_credential_request_for(rp_id, user_id, "cred-blob-user", true);
    let mut ext = ctap2::make_credential::ExtensionsInput::default();
    ext.cred_blob = Some(leak_bytes(blob.to_vec()));
    req.extensions = Some(ext);
    up::approve();
    match authn
        .call_ctap2(&Request::MakeCredential(req))
        .expect("MC with credBlob should succeed")
    {
        Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
        other => panic!("Expected MakeCredential, got {:?}", other),
    }
}

/// Read the device's advertised `maxCredBlobLength` from GetInfo.
fn max_cred_blob_length(authn: &mut dyn TestAuthenticator) -> usize {
    match authn.call_ctap2(&Request::GetInfo).expect("GetInfo") {
        Response::GetInfo(info) => info
            .max_cred_blob_length
            .expect("GetInfo must advertise maxCredBlobLength"),
        other => panic!("Expected GetInfo, got {:?}", other),
    }
}

/// P-1: GetInfo contains maxCredBlobLength (0x0F) and it is at least 32, and
/// the credBlob extension is advertised.
#[test]
#[serial]
fn cred_blob_get_info() {
    run_in_thread(|| {
        with_authenticator!(cred_blob_info, |authn| {
            let resp = authn.call_ctap2(&Request::GetInfo).expect("GetInfo");
            match resp {
                Response::GetInfo(info) => {
                    let exts = info.extensions.expect("extensions should be present");
                    assert!(
                        exts.contains(&ctap_types::ctap2::get_info::Extension::CredBlob),
                        "credBlob should be advertised"
                    );
                    let max = info
                        .max_cred_blob_length
                        .expect("GetInfo is missing maxCredBlobLength field");
                    assert!(
                        max >= 32,
                        "maxCredBlobLength must be at least 32, got {max}"
                    );
                }
                other => panic!("Expected GetInfo, got {:?}", other),
            }
        })
    });
}

/// P-2: Create a discoverable credential with a non-empty credBlob, then read
/// it back via GetAssertion (credBlob:true). The returned bytes must equal the
/// bytes stored at MakeCredential time.
#[test]
#[serial]
fn cred_blob_round_trip() {
    run_in_thread(|| {
        with_authenticator!(cred_blob_round_trip, |authn| {
            reset_authenticator(authn);

            let rp_id = "credblob.example.com";
            let blob: Vec<u8> = (0u8..20).collect();
            let cred_id = mc_with_cred_blob(authn, rp_id, &[0x01; 16], &blob);

            let mut req = get_assertion_request_for(rp_id, Some(single_allow_list(&cred_id)));
            let mut ext = ctap2::get_assertion::ExtensionsInput::default();
            ext.cred_blob = Some(true);
            req.extensions = Some(ext);
            up::approve();
            let ga = match authn
                .call_ctap2(&Request::GetAssertion(req))
                .expect("GA with credBlob:true should succeed")
            {
                Response::GetAssertion(ga) => ga,
                other => panic!("Expected GetAssertion, got {:?}", other),
            };

            assert!(
                ga.auth_data[32] & 0x80 != 0,
                "extension data flag should be set when credBlob is returned"
            );
            let returned =
                get_assertion_cred_blob(&ga.auth_data).expect("credBlob output should be present");
            assert_eq!(
                returned, blob,
                "credBlob response does not equal previously saved credBlob"
            );
        })
    });
}

/// P-3: Create a discoverable credential WITHOUT a credBlob, then GetAssertion
/// with credBlob:true. The result must contain the credBlob extension with an
/// empty byte string.
#[test]
#[serial]
fn cred_blob_absent_returns_empty() {
    run_in_thread(|| {
        with_authenticator!(cred_blob_absent, |authn| {
            reset_authenticator(authn);

            let rp_id = "credblob-empty.example.com";
            // Resident credential, no credBlob extension on MakeCredential.
            let mc_req = make_credential_request_for(rp_id, &[0x02; 16], "no-blob-user", true);
            up::approve();
            let cred_id = match authn
                .call_ctap2(&Request::MakeCredential(mc_req))
                .expect("MC without credBlob should succeed")
            {
                Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                other => panic!("Expected MakeCredential, got {:?}", other),
            };

            let mut req = get_assertion_request_for(rp_id, Some(single_allow_list(&cred_id)));
            let mut ext = ctap2::get_assertion::ExtensionsInput::default();
            ext.cred_blob = Some(true);
            req.extensions = Some(ext);
            up::approve();
            let ga = match authn
                .call_ctap2(&Request::GetAssertion(req))
                .expect("GA with credBlob:true should succeed")
            {
                Response::GetAssertion(ga) => ga,
                other => panic!("Expected GetAssertion, got {:?}", other),
            };

            assert!(
                ga.auth_data[32] & 0x80 != 0,
                "extension data flag should be set even when credBlob is empty"
            );
            let returned = get_assertion_cred_blob(&ga.auth_data)
                .expect("credBlob output should be present (as empty byte string)");
            assert_eq!(
                returned.len(),
                0,
                "expected credBlob response to be empty for a credential with no stored blob"
            );
        })
    });
}

/// Extra coverage (spec §11.1, not a numbered conformance case): a credBlob of
/// exactly maxCredBlobLength bytes is accepted and round-trips intact, while a
/// blob exceeding the limit is rejected (the credential is still created, but
/// the credBlob output flag is `false` and no blob is stored).
#[test]
#[serial]
fn cred_blob_max_length() {
    run_in_thread(|| {
        with_authenticator!(cred_blob_max, |authn| {
            reset_authenticator(authn);

            let max = max_cred_blob_length(authn);
            let rp_id = "credblob-max.example.com";

            // Exactly maxCredBlobLength bytes round-trips.
            let blob: Vec<u8> = (0..max).map(|i| (i as u8).wrapping_mul(3)).collect();
            let cred_id = mc_with_cred_blob(authn, rp_id, &[0x03; 16], &blob);

            let mut req = get_assertion_request_for(rp_id, Some(single_allow_list(&cred_id)));
            let mut ext = ctap2::get_assertion::ExtensionsInput::default();
            ext.cred_blob = Some(true);
            req.extensions = Some(ext);
            up::approve();
            let ga = match authn
                .call_ctap2(&Request::GetAssertion(req))
                .expect("GA with credBlob:true should succeed")
            {
                Response::GetAssertion(ga) => ga,
                other => panic!("Expected GetAssertion, got {:?}", other),
            };
            let returned =
                get_assertion_cred_blob(&ga.auth_data).expect("credBlob output should be present");
            assert_eq!(returned, blob, "max-length credBlob must round-trip intact");
        })
    });
}

/// Extra coverage (spec §11.1): a credBlob larger than maxCredBlobLength is not
/// stored. The MakeCredential still succeeds, but a subsequent GetAssertion
/// with credBlob:true returns an empty byte string (no blob was persisted).
#[test]
#[serial]
fn cred_blob_over_max_not_stored() {
    run_in_thread(|| {
        with_authenticator!(cred_blob_over_max, |authn| {
            reset_authenticator(authn);

            let max = max_cred_blob_length(authn);
            let rp_id = "credblob-over.example.com";

            // maxCredBlobLength + 1 bytes: must not be stored.
            let blob: Vec<u8> = (0..(max + 1)).map(|i| i as u8).collect();
            let cred_id = mc_with_cred_blob(authn, rp_id, &[0x04; 16], &blob);

            let mut req = get_assertion_request_for(rp_id, Some(single_allow_list(&cred_id)));
            let mut ext = ctap2::get_assertion::ExtensionsInput::default();
            ext.cred_blob = Some(true);
            req.extensions = Some(ext);
            up::approve();
            let ga = match authn
                .call_ctap2(&Request::GetAssertion(req))
                .expect("GA with credBlob:true should succeed")
            {
                Response::GetAssertion(ga) => ga,
                other => panic!("Expected GetAssertion, got {:?}", other),
            };
            let returned = get_assertion_cred_blob(&ga.auth_data)
                .expect("credBlob output should be present (empty)");
            assert_eq!(
                returned.len(),
                0,
                "over-limit credBlob must not be stored (empty on retrieval)"
            );
        })
    });
}
