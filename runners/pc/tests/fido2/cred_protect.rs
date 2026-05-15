//! CredProtect extension tests.

use super::*;
use support::pin::PinSession;

fn mc_with_cred_protect(authn: &mut dyn TestAuthenticator, level: u8) -> Vec<u8> {
    let mut req = make_credential_request_for(
        "credprotect.example.com",
        &[level; 16],
        &format!("cp-{level}"),
        true,
    );
    let mut ext = ctap2::make_credential::ExtensionsInput::default();
    ext.cred_protect = Some(level);
    req.extensions = Some(ext);
    up::approve();
    match authn
        .call_ctap2(&Request::MakeCredential(req))
        .unwrap_or_else(|e| panic!("MC with credProtect={level} should succeed: {e:?}"))
    {
        Response::MakeCredential(mc) => {
            // Extension data flag (bit 7) should be set
            assert!(
                mc.auth_data[32] & 0x80 != 0,
                "extension data flag should be set for credProtect={level}"
            );
            extract_credential_id(&mc.auth_data)
        }
        other => panic!("Expected MakeCredential, got {:?}", other),
    }
}

/// CredProtect levels 1-3: creation, exclusion visibility, and assertion behavior.
#[test]
#[serial]
fn cred_protect_group() {
    run_in_thread(|| {
        with_authenticator!(cred_protect, |authn| {
            reset_authenticator(authn);

            // Create credentials at each protection level
            let cred_optional = mc_with_cred_protect(authn, 1);
            let cred_optional_list = mc_with_cred_protect(authn, 2);
            let cred_required = mc_with_cred_protect(authn, 3);

            // --- Level 1 (optional) should be visible in exclude list without UV ---
            {
                let mut req = make_credential_request_for(
                    "credprotect.example.com",
                    &[0xA1; 16],
                    "cp-excl-1",
                    true,
                );
                let mut list = ctap_types::Vec::new();
                list.push(descriptor_ref(&cred_optional)).unwrap();
                req.exclude_list = Some(list);
                up::approve();
                let result = authn.call_ctap2(&Request::MakeCredential(req));
                assert_eq!(
                    result,
                    Err(ctap2::Error::CredentialExcluded),
                    "credProtect=1 should be excluded without UV"
                );
            }

            // --- Level 2 (optional+list) should be visible in exclude list without UV ---
            {
                let mut req = make_credential_request_for(
                    "credprotect.example.com",
                    &[0xA2; 16],
                    "cp-excl-2",
                    true,
                );
                let mut list = ctap_types::Vec::new();
                list.push(descriptor_ref(&cred_optional_list)).unwrap();
                req.exclude_list = Some(list);
                up::approve();
                let result = authn.call_ctap2(&Request::MakeCredential(req));
                assert_eq!(
                    result,
                    Err(ctap2::Error::CredentialExcluded),
                    "credProtect=2 should be excluded without UV"
                );
            }

            // --- Level 3 (required) should NOT be visible in exclude list without UV ---
            {
                let mut req = make_credential_request_for(
                    "credprotect.example.com",
                    &[0xA3; 16],
                    "cp-excl-3",
                    true,
                );
                let mut list = ctap_types::Vec::new();
                list.push(descriptor_ref(&cred_required)).unwrap();
                req.exclude_list = Some(list);
                up::approve();
                // Should succeed (not excluded) because level 3 requires UV to be visible
                authn
                    .call_ctap2(&Request::MakeCredential(req))
                    .expect("credProtect=3 should NOT be excluded without UV");
            }

            // --- Level 1 discoverable without allow list ---
            {
                up::approve();
                let resp = authn.call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                    "credprotect.example.com",
                    None,
                )));
                // Should succeed — at least the level 1 and 2 creds are discoverable
                assert!(
                    resp.is_ok(),
                    "discoverable assertion should work (level 1+2 visible)"
                );
            }

            // --- Allow list with all 3 creds, no UV: level 3 should be filtered out ---
            {
                let mut allow_list: ctap2::get_assertion::AllowList<'static> =
                    ctap_types::Vec::new();
                for cred in [&cred_optional, &cred_optional_list, &cred_required] {
                    allow_list.push(descriptor_ref(cred)).unwrap();
                }

                up::approve();
                let resp = authn
                    .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                        "credprotect.example.com",
                        Some(allow_list),
                    )))
                    .expect("allow list GA should succeed");

                match resp {
                    Response::GetAssertion(ga) => {
                        // number_of_credentials should be None (CTAP2.1 with allow list) or 2 (level 3 filtered)
                        if let Some(count) = ga.number_of_credentials {
                            assert_eq!(count, 2, "level 3 should be filtered out without UV");
                        }
                        // The returned credential should NOT be the level 3 one
                        assert_ne!(
                            ga.credential.id.to_vec(),
                            cred_required,
                            "level 3 credential should not be returned without UV"
                        );
                    }
                    other => panic!("Expected GetAssertion, got {:?}", other),
                }
            }
        })
    });
}

/// CTAP 2.1 credProtect §12.1: with UV performed via pinUvAuthParam, all
/// three protection levels are accessible in an allow list (level 3 is no
/// longer filtered out).
#[test]
#[serial]
fn cred_protect_all_levels_visible_with_uv() {
    run_isolated_in_sim(
        "cred_protect::cred_protect_all_levels_visible_with_uv",
        || {
            run_in_thread(|| {
                with_authenticator!(cred_protect_uv, |authn| {
                    reset_authenticator(authn);

                    let cred1 = mc_with_cred_protect(authn, 1);
                    let cred2 = mc_with_cred_protect(authn, 2);
                    let cred3 = mc_with_cred_protect(authn, 3);

                    const PIN: &str = "123456A";
                    PinSession::set_pin(authn, PIN);
                    let pin = PinSession::get_pin_token(authn, PIN);

                    let mut allow_list: ctap2::get_assertion::AllowList<'static> =
                        ctap_types::Vec::new();
                    for cred in [&cred1, &cred2, &cred3] {
                        allow_list.push(descriptor_ref(cred)).unwrap();
                    }

                    let mut req =
                        get_assertion_request_for("credprotect.example.com", Some(allow_list));
                    req.pin_protocol = Some(pin.protocol().into());
                    let pin_auth = pin.pin_auth_for_client_data_hash(req.client_data_hash.as_ref());
                    req.pin_auth = Some(leak_bytes(pin_auth.to_vec()));

                    up::approve();
                    let resp = authn
                        .call_ctap2(&Request::GetAssertion(req))
                        .expect("GA with UV across allow list should succeed");
                    match resp {
                        Response::GetAssertion(ga) => {
                            // CTAP 2.1 allows omission of numberOfCredentials when
                            // the host already knows the count via an allow list.
                            // If present, all 3 must be visible.
                            if let Some(count) = ga.number_of_credentials {
                                assert_eq!(
                                    count, 3,
                                    "with UV, all 3 cred-protect levels must be visible",
                                );
                            }
                        }
                        other => panic!("Expected GetAssertion, got {:?}", other),
                    }
                })
            });
        },
    );
}

/// Combined extensions: credProtect + hmac-secret in one MC request.
#[test]
#[serial]
fn cred_protect_with_hmac_secret() {
    run_in_thread(|| {
        with_authenticator!(cp_hmac, |authn| {
            reset_authenticator(authn);

            let mut req =
                make_credential_request_for("combined.example.com", &[0xBB; 16], "combined", false);
            let mut ext = ctap2::make_credential::ExtensionsInput::default();
            ext.cred_protect = Some(1);
            ext.hmac_secret = Some(true);
            req.extensions = Some(ext);
            up::approve();
            let resp = authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("MC with credProtect+hmac-secret should succeed");
            match resp {
                Response::MakeCredential(mc) => {
                    // Extension data flag should be set
                    assert!(
                        mc.auth_data[32] & 0x80 != 0,
                        "extension data flag should be set"
                    );
                }
                other => panic!("Expected MakeCredential, got {:?}", other),
            }
        })
    });
}
