//! largeBlob extension tests (per-credential, rides on largeBlobKey).
//!
//! Ports the FIDO CTAP2.3 conformance category
//! `tests/CTAP2/Protocol/Extensions/largeBlob.js` (request-building logic in
//! `js/LargeBlob2.1.js` / `js/CTAP2.js`).
//!
//! IMPORTANT — applicability to this device:
//!
//! The conformance suite distinguishes the high-level `largeBlob` extension
//! (which carries `support`/`read`/`write`/`originalSize` and returns
//! `unsignedExtensionOutputs(0x06/0x08)`) from the low-level `largeBlobKey`
//! extension (which only returns a 32-byte per-credential key). The suite's
//! `before()` hook asserts the mutual exclusion:
//!
//!   "If authenticator supports LargeBlob extension then it must NOT support
//!    LargeBlobKey extension."
//!
//! and `this.skip()`s the entire `largeBlob.js` category when `largeBlob` is
//! NOT advertised (largeBlob.js:48-50).
//!
//! Our device advertises `largeBlobKey` and does NOT advertise `largeBlob`
//! (see ctap-types `get_info::Extension` — there is no `LargeBlob` variant, and
//! `make_credential::ExtensionsInput` / `get_assertion::ExtensionsInput` have a
//! `large_blob_key` field but no `large_blob` field; the
//! `UnsignedExtensionOutputs` structs are empty `{}`). The firmware therefore
//! does not implement `largeBlob` `support`/`read`/`write` at all — the FIDO
//! tool skips this whole category for us.
//!
//! Accordingly:
//!   * `largeblob_extension_not_advertised` is the one test that actually
//!     asserts something for our device: it mirrors the `before()` invariant
//!     (largeBlobKey present, largeBlob absent). It runs on every backend.
//!   * P-1, P-3..P-5 and F-1, F-3..F-5 are ported as DEVICE-ONLY raw-CBOR
//!     requests so they compile and trivially pass in sim/CI (gated with
//!     `if !transport::is_device_mode() { return; }`) and exercise the wire
//!     path on a real device. Because the firmware ignores the unknown
//!     `largeBlob` extension, these are structured around the request path
//!     rather than asserting `largeBlob` output semantics. See the per-test
//!     notes and the module-level note in the integrator summary.
//!
//! Skipped cases: P-2 (duplicate of P-1, "required" vs "preferred"), P-6/P-7
//! (the largeBlob-enabled-by-default branch question — the device has no
//! largeBlob support at all), F-2 (duplicate of F-1/F-3 GA invalid-CBOR path).

use super::*;
use support::pin::PinSession;

/// CBOR for a MakeCredential request (cmd 0x01 payload) with an optional
/// `largeBlob` extension map (request field 0x06 = extensions). rk=true.
///
/// pinUvAuthParam (0x08) / pinUvAuthProtocol (0x09) are attached when a PIN
/// session is supplied (the conformance suite always uses a PUAT here).
fn make_credential_cbor(
    rp_id: &str,
    user_id: &[u8],
    pin: Option<&PinSession>,
    large_blob_ext: Option<Value>,
) -> Vec<u8> {
    use ctap_types::webauthn::*;

    let client_data_hash = [0xcd_u8; 32];

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
        name: Some("lbuser".try_into().unwrap()),
        display_name: Some("Large Blob User".try_into().unwrap()),
    };

    let mut entries = vec![
        (Value::Integer(1), Value::Bytes(client_data_hash.to_vec())),
        (
            Value::Integer(2),
            serde_cbor::value::to_value(&rp).expect("serialize rp"),
        ),
        (
            Value::Integer(3),
            serde_cbor::value::to_value(&user).expect("serialize user"),
        ),
        (
            Value::Integer(4),
            serde_cbor::value::to_value(&params).expect("serialize params"),
        ),
    ];

    if let Some(ext) = large_blob_ext {
        // 0x06 = extensions; { "largeBlob": <struct> }
        let ext_map = Value::Map(
            [(Value::Text("largeBlob".to_string()), ext)]
                .into_iter()
                .collect(),
        );
        entries.push((Value::Integer(6), ext_map));
    }

    // 0x07 = options { rk: true }
    entries.push((
        Value::Integer(7),
        Value::Map(
            [(Value::Text("rk".to_string()), Value::Bool(true))]
                .into_iter()
                .collect(),
        ),
    ));

    if let Some(pin) = pin {
        let pin_auth = pin.pin_auth_for_client_data_hash(&client_data_hash);
        entries.push((Value::Integer(8), Value::Bytes(pin_auth.to_vec())));
        entries.push((Value::Integer(9), Value::Integer(pin.protocol() as i128)));
    }

    serde_cbor::to_vec(&Value::Map(entries.into_iter().collect()))
        .expect("serialize makeCredential CBOR")
}

/// CBOR for a GetAssertion request (cmd 0x02 payload) with an optional
/// `largeBlob` extension map (request field 0x04 = extensions) and an optional
/// single-credential allow list (field 0x03).
fn get_assertion_cbor(
    rp_id: &str,
    cred_id: Option<&[u8]>,
    pin: Option<&PinSession>,
    large_blob_ext: Option<Value>,
) -> Vec<u8> {
    let client_data_hash = [0xcd_u8; 32];

    let mut entries = vec![
        (Value::Integer(1), Value::Text(rp_id.to_string())),
        (Value::Integer(2), Value::Bytes(client_data_hash.to_vec())),
    ];

    if let Some(cred_id) = cred_id {
        // 0x03 = allowList: [ { type: "public-key", id: <cred_id> } ]
        let descriptor = Value::Map(
            [
                (
                    Value::Text("type".to_string()),
                    Value::Text("public-key".to_string()),
                ),
                (
                    Value::Text("id".to_string()),
                    Value::Bytes(cred_id.to_vec()),
                ),
            ]
            .into_iter()
            .collect(),
        );
        entries.push((Value::Integer(3), Value::Array(vec![descriptor])));
    }

    if let Some(ext) = large_blob_ext {
        // 0x04 = extensions; { "largeBlob": <struct> }
        let ext_map = Value::Map(
            [(Value::Text("largeBlob".to_string()), ext)]
                .into_iter()
                .collect(),
        );
        entries.push((Value::Integer(4), ext_map));
    }

    if let Some(pin) = pin {
        let pin_auth = pin.pin_auth_for_client_data_hash(&client_data_hash);
        entries.push((Value::Integer(6), Value::Bytes(pin_auth.to_vec())));
        entries.push((Value::Integer(7), Value::Integer(pin.protocol() as i128)));
    }

    serde_cbor::to_vec(&Value::Map(entries.into_iter().collect()))
        .expect("serialize getAssertion CBOR")
}

/// Extract the credential id from the authData inside a MakeCredential response
/// CBOR map (response field 0x02 = authData byte string; 0x01 is `fmt`).
fn cred_id_from_mc_response(body: &[u8]) -> Vec<u8> {
    let value: Value = serde_cbor::from_slice(body).expect("decode MC response CBOR");
    let auth_data = match value {
        Value::Map(m) => m
            .into_iter()
            .find_map(|(k, v)| match (k, v) {
                (Value::Integer(2), Value::Bytes(b)) => Some(b),
                _ => None,
            })
            .expect("MC response missing authData(0x02)"),
        other => panic!("MC response is not a map: {other:?}"),
    };
    extract_credential_id(&auth_data)
}

/// Establish a PIN and return a PUAT with make-credential + get-assertion
/// permissions (the conformance suite uses a per-RP PUAT throughout).
fn setup_pin(authn: &mut dyn TestAuthenticator) -> PinSession {
    const PIN: &str = "123456A";
    // Idempotent: these tests call setup_pin before both the MC and the GA, but a
    // second SetPin while a PIN already exists fails PinAuthInvalid (correct
    // firmware behavior). Set once, ignore the "already set" on later calls, then
    // mint a fresh PUAT against the live PIN (try_get_pin_token validates it).
    let _ = PinSession::try_set_pin(authn, PIN);
    let perms = ctap2::client_pin::Permissions::MAKE_CREDENTIAL
        | ctap2::client_pin::Permissions::GET_ASSERTION;
    // Device mode: under heavy probe-rs/SWD traffic the PUAT handshake
    // (getKeyAgreement + getPinToken) can intermittently observe a rotated
    // keyAgreement key; retry once with a fresh handshake.
    if let Ok(p) = PinSession::try_get_pin_token_with_permissions(authn, PIN, perms) {
        return p;
    }
    PinSession::try_get_pin_token_with_permissions(authn, PIN, perms)
        .expect("get PUAT with mc+ga permissions (after retry)")
}

/// `before()` invariant (largeBlob.js:36-50): the device must advertise
/// `largeBlobKey` and must NOT advertise the high-level `largeBlob` extension
/// (the two are mutually exclusive). This is the one assertion that holds for
/// our device, so it runs on every backend.
#[test]
#[serial]
fn largeblob_extension_not_advertised() {
    run_in_thread(|| {
        with_authenticator!(lb_ext_advertised, |authn| {
            let info = match authn.call_ctap2(&Request::GetInfo).expect("GetInfo failed") {
                Response::GetInfo(info) => info,
                other => panic!("Expected GetInfo, got {:?}", other),
            };
            let extensions: Vec<String> = info
                .extensions
                .map(|exts| exts.iter().map(|e| <&str>::from(*e).to_string()).collect())
                .unwrap_or_default();

            // The `largeBlobKey` extension is only advertised when the
            // authenticator is configured with largeBlobs storage. The sim's
            // `with_authenticator!` Config hardcodes `large_blobs: None`, so it
            // does NOT advertise `largeBlobKey`; only the real device does.
            // The half of the invariant that holds on EVERY backend is that the
            // high-level `largeBlob` extension is never advertised (this
            // implementation has no largeBlob extension support at all).
            if transport::is_device_mode() {
                assert!(
                    extensions.iter().any(|e| e == "largeBlobKey"),
                    "device should advertise largeBlobKey; got {extensions:?}"
                );
            }
            assert!(
                !extensions.iter().any(|e| e == "largeBlob"),
                "authenticator must NOT advertise largeBlob when largeBlobKey is supported \
                 (CTAP2.3 largeBlob.js before() invariant); got {extensions:?}"
            );
        })
    });
}

/// P-1: MakeCredential with largeBlob `support: "preferred"`.
///
/// DEVICE-ONLY. The firmware does not implement the largeBlob extension, so the
/// unknown extension is ignored and the credential is still created. We assert
/// the request path succeeds. (Asserting `unsignedExtensionOutputs.largeBlob.
/// supported == true` would require firmware support the device does not have.)
#[test]
#[serial]
fn p1_make_credential_largeblob_support_preferred() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_p1, |authn| {
            reset_authenticator(authn);
            let pin = setup_pin(authn);

            let ext = Value::Map(
                [(
                    Value::Text("support".to_string()),
                    Value::Text("preferred".to_string()),
                )]
                .into_iter()
                .collect(),
            );
            let payload =
                make_credential_cbor("largeblob.example.com", &[0x01; 16], Some(&pin), Some(ext));

            up::approve();
            let (status, body) = authn
                .call_ctap2_raw(0x01, &payload)
                .expect("transport error");
            assert_eq!(
                status, 0x00,
                "MakeCredential with largeBlob support=preferred should succeed (status {status:#04x})"
            );
            // Credential is created and has a valid id.
            assert!(
                !cred_id_from_mc_response(&body).is_empty(),
                "MakeCredential should return a credential id"
            );
        })
    });
}

/// P-3: GetAssertion with largeBlob `write` + `originalSize` against a freshly
/// registered credential (allow list present).
///
/// DEVICE-ONLY. The firmware ignores the unknown extension; we assert the
/// assertion request path succeeds.
#[test]
#[serial]
fn p3_get_assertion_largeblob_write() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_p3, |authn| {
            reset_authenticator(authn);
            let pin = setup_pin(authn);

            // Register a credential first.
            let mc_payload =
                make_credential_cbor("largeblob.example.com", &[0x03; 16], Some(&pin), None);
            up::approve();
            let (mc_status, mc_body) = authn
                .call_ctap2_raw(0x01, &mc_payload)
                .expect("transport error");
            assert_eq!(mc_status, 0x00, "MakeCredential should succeed");
            let cred_id = cred_id_from_mc_response(&mc_body);

            // GetAssertion with largeBlob write.
            let write_bstr = vec![0xABu8; 32];
            let ext = Value::Map(
                [
                    (
                        Value::Text("write".to_string()),
                        Value::Bytes(write_bstr.clone()),
                    ),
                    (
                        Value::Text("originalSize".to_string()),
                        Value::Integer(write_bstr.len() as i128),
                    ),
                ]
                .into_iter()
                .collect(),
            );
            let ga_payload = get_assertion_cbor(
                "largeblob.example.com",
                Some(&cred_id),
                Some(&pin),
                Some(ext),
            );
            up::approve();
            let (ga_status, _) = authn
                .call_ctap2_raw(0x02, &ga_payload)
                .expect("transport error");
            assert_eq!(
                ga_status, 0x00,
                "GetAssertion with largeBlob write should succeed (status {ga_status:#04x})"
            );
        })
    });
}

/// P-4: GetAssertion with largeBlob `read: true`.
///
/// DEVICE-ONLY. Request-path only (firmware has no largeBlob storage).
#[test]
#[serial]
fn p4_get_assertion_largeblob_read() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_p4, |authn| {
            reset_authenticator(authn);
            let pin = setup_pin(authn);

            let mc_payload =
                make_credential_cbor("largeblob.example.com", &[0x04; 16], Some(&pin), None);
            up::approve();
            let (mc_status, mc_body) = authn
                .call_ctap2_raw(0x01, &mc_payload)
                .expect("transport error");
            assert_eq!(mc_status, 0x00, "MakeCredential should succeed");
            let cred_id = cred_id_from_mc_response(&mc_body);

            let ext = Value::Map(
                [(Value::Text("read".to_string()), Value::Bool(true))]
                    .into_iter()
                    .collect(),
            );
            let ga_payload = get_assertion_cbor(
                "largeblob.example.com",
                Some(&cred_id),
                Some(&pin),
                Some(ext),
            );
            up::approve();
            let (ga_status, _) = authn
                .call_ctap2_raw(0x02, &ga_payload)
                .expect("transport error");
            assert_eq!(
                ga_status, 0x00,
                "GetAssertion with largeBlob read=true should succeed (status {ga_status:#04x})"
            );
        })
    });
}

/// P-5: GetAssertion with largeBlob `write` but NO allow list. Per spec the
/// write fails (`written: false`) because the credential is ambiguous.
///
/// DEVICE-ONLY. The discoverable assertion still completes; we assert the
/// request path succeeds.
#[test]
#[serial]
fn p5_get_assertion_largeblob_write_no_allowlist() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_p5, |authn| {
            reset_authenticator(authn);
            let pin = setup_pin(authn);

            // Register a discoverable credential.
            let mc_payload =
                make_credential_cbor("largeblob.example.com", &[0x05; 16], Some(&pin), None);
            up::approve();
            let (mc_status, _) = authn
                .call_ctap2_raw(0x01, &mc_payload)
                .expect("transport error");
            assert_eq!(mc_status, 0x00, "MakeCredential should succeed");

            // Fresh PUAT for the GA, then write with no allow list.
            let pin = setup_pin(authn);
            let write_bstr = vec![0xCDu8; 32];
            let ext = Value::Map(
                [
                    (
                        Value::Text("write".to_string()),
                        Value::Bytes(write_bstr.clone()),
                    ),
                    (
                        Value::Text("originalSize".to_string()),
                        Value::Integer(write_bstr.len() as i128),
                    ),
                ]
                .into_iter()
                .collect(),
            );
            let ga_payload =
                get_assertion_cbor("largeblob.example.com", None, Some(&pin), Some(ext));
            up::approve();
            let (ga_status, _) = authn
                .call_ctap2_raw(0x02, &ga_payload)
                .expect("transport error");
            assert_eq!(
                ga_status, 0x00,
                "discoverable GetAssertion with largeBlob write (no allow list) should still \
                 complete the assertion (status {ga_status:#04x})"
            );
        })
    });
}

/// F-1: MakeCredential with an invalid largeBlob map `{ "wrong": "123" }`.
/// Spec expects `CTAP2_ERR_INVALID_CBOR (0x12)`.
///
/// DEVICE-ONLY. The firmware has no largeBlob parser, so it ignores the unknown
/// extension and the request succeeds — this case cannot fail with INVALID_CBOR
/// on our device. We accept either success (ignored) or INVALID_CBOR; on a
/// largeBlob-supporting authenticator it would be INVALID_CBOR.
#[test]
#[serial]
fn f1_make_credential_invalid_largeblob_map() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_f1, |authn| {
            reset_authenticator(authn);
            let pin = setup_pin(authn);

            let ext = Value::Map(
                [(
                    Value::Text("wrong".to_string()),
                    Value::Text("123".to_string()),
                )]
                .into_iter()
                .collect(),
            );
            let payload =
                make_credential_cbor("largeblob.example.com", &[0xF1; 16], Some(&pin), Some(ext));
            up::approve();
            let (status, _) = authn
                .call_ctap2_raw(0x01, &payload)
                .expect("transport error");
            assert!(
                status == 0x00 || status == 0x12,
                "invalid largeBlob map: expected success (extension ignored) or \
                 INVALID_CBOR(0x12), got {status:#04x}"
            );
        })
    });
}

/// F-3: GetAssertion with largeBlob containing `read` + `write` + `originalSize`
/// simultaneously (mutually exclusive). Spec expects INVALID_CBOR(0x12).
///
/// DEVICE-ONLY. Same caveat as F-1: the firmware ignores the unknown extension.
#[test]
#[serial]
fn f3_get_assertion_largeblob_read_and_write() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_f3, |authn| {
            reset_authenticator(authn);
            let pin = setup_pin(authn);

            let mc_payload =
                make_credential_cbor("largeblob.example.com", &[0xF3; 16], Some(&pin), None);
            up::approve();
            let (mc_status, mc_body) = authn
                .call_ctap2_raw(0x01, &mc_payload)
                .expect("transport error");
            assert_eq!(mc_status, 0x00, "MakeCredential should succeed");
            let cred_id = cred_id_from_mc_response(&mc_body);

            let pin = setup_pin(authn);
            let ext = Value::Map(
                [
                    (Value::Text("read".to_string()), Value::Bool(true)),
                    (
                        Value::Text("write".to_string()),
                        Value::Bytes(vec![0x11; 32]),
                    ),
                    (Value::Text("originalSize".to_string()), Value::Integer(32)),
                ]
                .into_iter()
                .collect(),
            );
            let ga_payload = get_assertion_cbor(
                "largeblob.example.com",
                Some(&cred_id),
                Some(&pin),
                Some(ext),
            );
            up::approve();
            let (status, _) = authn
                .call_ctap2_raw(0x02, &ga_payload)
                .expect("transport error");
            assert!(
                status == 0x00 || status == 0x12,
                "read+write+originalSize together: expected success (extension ignored) or \
                 INVALID_CBOR(0x12), got {status:#04x}"
            );
        })
    });
}

/// F-4: GetAssertion with largeBlob `read` set to a non-boolean type (here a
/// text string). Spec expects INVALID_CBOR(0x12).
///
/// DEVICE-ONLY. Same caveat: the firmware ignores the unknown extension.
#[test]
#[serial]
fn f4_get_assertion_largeblob_read_invalid_type() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_f4, |authn| {
            reset_authenticator(authn);
            let pin = setup_pin(authn);

            let mc_payload =
                make_credential_cbor("largeblob.example.com", &[0xF4; 16], Some(&pin), None);
            up::approve();
            let (mc_status, mc_body) = authn
                .call_ctap2_raw(0x01, &mc_payload)
                .expect("transport error");
            assert_eq!(mc_status, 0x00, "MakeCredential should succeed");
            let cred_id = cred_id_from_mc_response(&mc_body);

            let pin = setup_pin(authn);
            let ext = Value::Map(
                [(
                    Value::Text("read".to_string()),
                    Value::Text("not-a-bool".to_string()),
                )]
                .into_iter()
                .collect(),
            );
            let ga_payload = get_assertion_cbor(
                "largeblob.example.com",
                Some(&cred_id),
                Some(&pin),
                Some(ext),
            );
            up::approve();
            let (status, _) = authn
                .call_ctap2_raw(0x02, &ga_payload)
                .expect("transport error");
            assert!(
                status == 0x00 || status == 0x12,
                "read=<non-bool>: expected success (extension ignored) or \
                 INVALID_CBOR(0x12), got {status:#04x}"
            );
        })
    });
}

/// F-5: GetAssertion with largeBlob `write`/`originalSize` set to invalid types
/// (write as a text string, originalSize as a text string). Spec expects
/// INVALID_CBOR(0x12).
///
/// DEVICE-ONLY. Same caveat: the firmware ignores the unknown extension.
#[test]
#[serial]
fn f5_get_assertion_largeblob_write_invalid_types() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_f5, |authn| {
            reset_authenticator(authn);
            let pin = setup_pin(authn);

            let mc_payload =
                make_credential_cbor("largeblob.example.com", &[0xF5; 16], Some(&pin), None);
            up::approve();
            let (mc_status, mc_body) = authn
                .call_ctap2_raw(0x01, &mc_payload)
                .expect("transport error");
            assert_eq!(mc_status, 0x00, "MakeCredential should succeed");
            let cred_id = cred_id_from_mc_response(&mc_body);

            let pin = setup_pin(authn);
            let ext = Value::Map(
                [
                    (
                        Value::Text("write".to_string()),
                        Value::Text("not-bytes".to_string()),
                    ),
                    (
                        Value::Text("originalSize".to_string()),
                        Value::Text("not-a-number".to_string()),
                    ),
                ]
                .into_iter()
                .collect(),
            );
            let ga_payload = get_assertion_cbor(
                "largeblob.example.com",
                Some(&cred_id),
                Some(&pin),
                Some(ext),
            );
            up::approve();
            let (status, _) = authn
                .call_ctap2_raw(0x02, &ga_payload)
                .expect("transport error");
            assert!(
                status == 0x00 || status == 0x12,
                "write/originalSize wrong types: expected success (extension ignored) or \
                 INVALID_CBOR(0x12), got {status:#04x}"
            );
        })
    });
}
