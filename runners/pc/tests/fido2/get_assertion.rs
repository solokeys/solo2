//! GetAssertion tests.

use super::*;

#[test]
#[serial]
fn ga_group_basic() {
    run_in_thread(|| {
        with_authenticator!(ga_group, |authn| {
            reset_authenticator(authn);

            up::approve();
            let cred_id = make_credential(authn);

            // --- basic GA ---
            up::approve();
            let resp = authn
                .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                    "example.com",
                    Some(single_allow_list(&cred_id)),
                )))
                .expect("GA should succeed");
            match resp {
                Response::GetAssertion(ga) => {
                    assert_eq!(ga.auth_data.len(), 37);
                    let flags = ga.auth_data[32];
                    assert!(flags & 0x40 == 0, "AT flag should NOT be set");
                    assert!(flags & 0x01 != 0, "UP flag should be set");
                    assert!(!ga.signature.is_empty());
                    // user and numberOfCredentials should not be present for single non-RK
                    assert!(
                        ga.user.is_none(),
                        "user should not be returned for non-RK single credential"
                    );
                    assert!(
                        ga.number_of_credentials.is_none(),
                        "numberOfCredentials should not be returned"
                    );
                }
                other => panic!("Expected GetAssertion, got {:?}", other),
            }

            // --- corrupt credential ID ---
            let mut bad_id = cred_id.clone();
            if let Some(b) = bad_id.last_mut() {
                *b ^= 0xFF;
            }
            assert!(
                authn
                    .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                        "example.com",
                        Some(single_allow_list(&bad_id))
                    )))
                    .is_err(),
                "corrupt cred ID should fail"
            );

            // --- wrong RP ---
            assert!(
                authn
                    .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                        "wrong.com",
                        Some(single_allow_list(&cred_id))
                    )))
                    .is_err(),
                "wrong RP should fail"
            );

            // --- empty allow list ---
            assert!(
                authn
                    .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                        "example.com",
                        Some(ctap_types::Vec::new())
                    )))
                    .is_err(),
                "empty allow list should fail"
            );

            // --- missing RP ---
            assert!(
                authn
                    .call_ctap2(&Request::GetAssertion(get_assertion_request_for("", None)))
                    .is_err(),
                "empty RP should fail"
            );

            // --- UP option false ---
            {
                let mut req =
                    get_assertion_request_for("example.com", Some(single_allow_list(&cred_id)));
                req.options = Some(decode_from_value(options_value(None, Some(false), None)));
                let resp = authn
                    .call_ctap2(&Request::GetAssertion(req))
                    .expect("GA with up=false should succeed");
                match resp {
                    Response::GetAssertion(ga) => {
                        // UP flag should NOT be set when up=false
                        assert!(
                            ga.auth_data[32] & 0x01 == 0,
                            "UP flag should not be set with up=false"
                        );
                    }
                    other => panic!("Expected GetAssertion, got {:?}", other),
                }
            }
        })
    });
}

/// GA with `uv=true` and no PIN configured must return InvalidOption
/// (CTAP 2.1 §6.2.2 step 3: uv requested without UV-supporting state).
#[test]
#[serial]
fn ga_option_uv_without_pin_is_invalid() {
    run_in_thread(|| {
        with_authenticator!(ga_option_uv_no_pin, |authn| {
            reset_authenticator(authn);
            up::approve();
            let cred_id = make_credential(authn);

            let mut req =
                get_assertion_request_for("example.com", Some(single_allow_list(&cred_id)));
            req.options = Some(decode_from_value(options_value(None, None, Some(true))));
            up::approve();
            assert_eq!(
                authn.call_ctap2(&Request::GetAssertion(req)),
                Err(ctap2::Error::InvalidOption),
                "uv=true without PIN must yield InvalidOption",
            );
        })
    });
}

/// Allow list filtering across multiple RPs with multiple credentials each.
#[test]
#[serial]
fn ga_allow_list_filtering() {
    run_in_thread(|| {
        with_authenticator!(ga_filter, |authn| {
            reset_authenticator(authn);

            // Register 3 credentials for rp1 and 3 for rp2
            let mut rp1_creds = Vec::new();
            let mut rp2_creds = Vec::new();

            for i in 0..3u8 {
                up::approve();
                let req = make_credential_request_for(
                    "rp1.example.com",
                    &[0x10 + i; 16],
                    &format!("rp1-user-{i}"),
                    false,
                );
                let resp = authn
                    .call_ctap2(&Request::MakeCredential(req))
                    .expect("MC rp1");
                match resp {
                    Response::MakeCredential(mc) => {
                        rp1_creds.push(extract_credential_id(&mc.auth_data))
                    }
                    other => panic!("{:?}", other),
                }
            }
            for i in 0..3u8 {
                up::approve();
                let req = make_credential_request_for(
                    "rp2.example.com",
                    &[0x20 + i; 16],
                    &format!("rp2-user-{i}"),
                    false,
                );
                let resp = authn
                    .call_ctap2(&Request::MakeCredential(req))
                    .expect("MC rp2");
                match resp {
                    Response::MakeCredential(mc) => {
                        rp2_creds.push(extract_credential_id(&mc.auth_data))
                    }
                    other => panic!("{:?}", other),
                }
            }

            // Build a combined allow list with all 6 credentials
            let mut all_creds: ctap2::get_assertion::AllowList<'static> = ctap_types::Vec::new();
            for cred in rp1_creds.iter().chain(rp2_creds.iter()) {
                all_creds.push(descriptor_ref(cred)).unwrap();
            }

            // GA for rp1 with combined allow list — should only match rp1 credentials
            up::approve();
            let resp = authn
                .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                    "rp1.example.com",
                    Some(all_creds.clone()),
                )))
                .expect("GA rp1 should succeed");
            match resp {
                Response::GetAssertion(ga) => {
                    let cred = ga.credential;
                    assert!(
                        rp1_creds.iter().any(|c| c == &cred.id.to_vec()),
                        "returned credential should be from rp1"
                    );
                }
                other => panic!("{:?}", other),
            }

            // GA for rp2 with combined allow list — should only match rp2 credentials
            up::approve();
            let resp = authn
                .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                    "rp2.example.com",
                    Some(all_creds),
                )))
                .expect("GA rp2 should succeed");
            match resp {
                Response::GetAssertion(ga) => {
                    let cred = ga.credential;
                    assert!(
                        rp2_creds.iter().any(|c| c == &cred.id.to_vec()),
                        "returned credential should be from rp2"
                    );
                }
                other => panic!("{:?}", other),
            }
        })
    });
}

/// Read the 4-byte signature counter from a GetAssertion authData (bytes 33..37).
fn auth_data_counter(auth_data: &[u8]) -> u32 {
    u32::from_be_bytes([auth_data[33], auth_data[34], auth_data[35], auth_data[36]])
}

/// Conformance Authr-GetAssertion-Req-2 P-2: GetAssertion with an explicit
/// `options.up=true` must succeed and set the UP flag in authData.
///
/// The existing `ga_group_basic` only exercises the `up=false` path; this adds
/// the positive `up=true` case so both branches of the up option are covered.
#[test]
#[serial]
fn ga_option_up_true_sets_up_flag() {
    run_in_thread(|| {
        with_authenticator!(ga_option_up_true, |authn| {
            reset_authenticator(authn);
            up::approve();
            let cred_id = make_credential(authn);

            let mut req =
                get_assertion_request_for("example.com", Some(single_allow_list(&cred_id)));
            req.options = Some(decode_from_value(options_value(None, Some(true), None)));
            up::approve();
            let resp = authn
                .call_ctap2(&Request::GetAssertion(req))
                .expect("GA with up=true should succeed");
            match resp {
                Response::GetAssertion(ga) => {
                    assert_eq!(ga.auth_data.len(), 37);
                    assert!(
                        ga.auth_data[32] & 0x01 != 0,
                        "UP flag must be set when options.up=true"
                    );
                }
                other => panic!("Expected GetAssertion, got {:?}", other),
            }
        })
    });
}

/// Conformance Authr-GetAssertion-Resp-1 P-3: three consecutive GetAssertion
/// requests for the same credential must return strictly increasing signature
/// counters (counterA < counterB < counterC).
#[test]
#[serial]
fn ga_counter_increases_across_requests() {
    run_in_thread(|| {
        with_authenticator!(ga_counter_increase, |authn| {
            reset_authenticator(authn);
            up::approve();
            let cred_id = make_credential(authn);

            let mut counters = [0u32; 3];
            for slot in counters.iter_mut() {
                up::approve();
                let resp = authn
                    .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                        "example.com",
                        Some(single_allow_list(&cred_id)),
                    )))
                    .expect("GA should succeed");
                match resp {
                    Response::GetAssertion(ga) => {
                        assert_eq!(ga.auth_data.len(), 37);
                        *slot = auth_data_counter(&ga.auth_data);
                    }
                    other => panic!("Expected GetAssertion, got {:?}", other),
                }
            }

            assert!(
                counters[0] < counters[1],
                "counter must increase: A({}) < B({})",
                counters[0],
                counters[1]
            );
            assert!(
                counters[1] < counters[2],
                "counter must increase: B({}) < C({})",
                counters[1],
                counters[2]
            );
        })
    });
}

/// Conformance Authr-GetAssertion-Resp-1 F-1: for a credential created without
/// extensions, the GetAssertion response must NOT carry an
/// `unsignedExtensionOutputs` field, and the extension-data (ED, bit 7) flag in
/// authData must be clear.
#[test]
#[serial]
fn ga_no_unsigned_extension_outputs_without_extensions() {
    run_in_thread(|| {
        with_authenticator!(ga_no_unsigned_ext, |authn| {
            reset_authenticator(authn);
            up::approve();
            let cred_id = make_credential(authn);

            up::approve();
            let resp = authn
                .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                    "example.com",
                    Some(single_allow_list(&cred_id)),
                )))
                .expect("GA should succeed");
            match resp {
                Response::GetAssertion(ga) => {
                    assert!(
                        ga.unsigned_extension_outputs.is_none(),
                        "unsignedExtensionOutputs must be absent for a credential created without extensions"
                    );
                    assert!(
                        ga.auth_data[32] & 0x80 == 0,
                        "ED (extension data) flag must not be set without extensions"
                    );
                }
                other => panic!("Expected GetAssertion, got {:?}", other),
            }
        })
    });
}
