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
