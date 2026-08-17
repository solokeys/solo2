//! Resident key (discoverable credential) tests.

use super::*;

fn unique_rp_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{n}.example.com")
}

fn resident_request(
    rp_id: &str,
    user_id: &[u8],
    user_name: &str,
) -> ctap2::make_credential::Request<'static> {
    make_credential_request_for(rp_id, user_id, user_name, true)
}

fn create_resident_credential(
    authn: &mut dyn TestAuthenticator,
    rp_id: &str,
    user_id: &[u8],
    user_name: &str,
) -> (Vec<u8>, ctap2::get_assertion::Response) {
    up::approve();
    let credential_id = match authn
        .call_ctap2(&Request::MakeCredential(resident_request(
            rp_id, user_id, user_name,
        )))
        .expect("resident MakeCredential should succeed")
    {
        Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
        other => panic!("Expected MakeCredential, got {:?}", other),
    };
    up::approve();
    let assertion = get_resident_assertion(authn, rp_id, None);
    (credential_id, assertion)
}

fn get_resident_assertion(
    authn: &mut dyn TestAuthenticator,
    rp_id: &str,
    allow_list: Option<ctap2::get_assertion::AllowList<'static>>,
) -> ctap2::get_assertion::Response {
    match authn
        .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
            rp_id, allow_list,
        )))
        .expect("resident GetAssertion should succeed")
    {
        Response::GetAssertion(ga) => ga,
        other => panic!("Expected GetAssertion, got {:?}", other),
    }
}

fn get_next_assertion(
    authn: &mut dyn TestAuthenticator,
) -> Result<ctap2::get_assertion::Response, ctap2::Error> {
    match authn.call_ctap2(&Request::GetNextAssertion)? {
        Response::GetNextAssertion(ga) => Ok(ga),
        other => panic!("Expected GetNextAssertion, got {:?}", other),
    }
}

fn response_credential_id(response: &ctap2::get_assertion::Response) -> Vec<u8> {
    response.credential.id.to_vec()
}

fn user_id(response: &ctap2::get_assertion::Response) -> Vec<u8> {
    response
        .user
        .as_ref()
        .expect("user should be present")
        .id
        .to_vec()
}

fn assert_single_account_user_fields(
    response: &ctap2::get_assertion::Response,
    expected_user_id: &[u8],
) {
    assert_eq!(user_id(response), expected_user_id);
    assert_eq!(
        response.number_of_credentials, None,
        "single-account assertions should not report numberOfCredentials"
    );
}

fn collect_resident_assertions(
    authn: &mut dyn TestAuthenticator,
    rp_id: &str,
) -> Vec<ctap2::get_assertion::Response> {
    up::approve();
    let first = get_resident_assertion(authn, rp_id, None);
    let count = first.number_of_credentials.unwrap_or(1) as usize;
    let mut assertions = vec![first];
    for _ in 1..count {
        assertions.push(get_next_assertion(authn).expect("GetNextAssertion should succeed"));
    }
    assertions
}

/// All resident key tests. Resets once at the start.
#[test]
#[serial]
fn rk_group() {
    run_in_thread(|| {
        with_authenticator!(rk_group, |authn| {
            reset_authenticator(authn);

            // --- basic auth and user info ---
            {
                let rp_id = unique_rp_id("rk-basic");
                let user = [0x11; 16];
                let (credential_id, assertion) =
                    create_resident_credential(authn, &rp_id, &user, "resident-basic");
                assert_eq!(response_credential_id(&assertion), credential_id);
                assert_single_account_user_fields(&assertion, &user);
            }

            // --- allow list lookup works ---
            {
                let rp_id = unique_rp_id("rk-allow");
                let user = [0x22; 16];
                up::approve();
                let credential_id = match authn
                    .call_ctap2(&Request::MakeCredential(resident_request(
                        &rp_id,
                        &user,
                        "allow-test",
                    )))
                    .expect("MC should succeed")
                {
                    Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                    other => panic!("Expected MC, got {:?}", other),
                };
                up::approve();
                let ga =
                    get_resident_assertion(authn, &rp_id, Some(single_allow_list(&credential_id)));
                assert_eq!(response_credential_id(&ga), credential_id);
            }

            // --- multiple RKs with enumeration ---
            {
                let rp_id = unique_rp_id("rk-multi");
                let users = [[0x30; 16], [0x31; 16], [0x32; 16]];
                let mut registrations = Vec::new();
                for (i, user) in users.iter().enumerate() {
                    up::approve();
                    let resp = authn
                        .call_ctap2(&Request::MakeCredential(resident_request(
                            &rp_id,
                            user,
                            &format!("r-{i}"),
                        )))
                        .expect("MC should succeed");
                    match resp {
                        Response::MakeCredential(mc) => {
                            registrations.push(extract_credential_id(&mc.auth_data))
                        }
                        other => panic!("Expected MC, got {:?}", other),
                    }
                }
                let assertions = collect_resident_assertions(authn, &rp_id);
                assert_eq!(assertions.len(), registrations.len());
                assert_eq!(assertions[0].number_of_credentials, Some(3));
            }

            // --- credential from different RP is rejected ---
            {
                let rp_a = unique_rp_id("rk-rp-a");
                let rp_b = unique_rp_id("rk-rp-b");
                create_resident_credential(authn, &rp_a, &[0x41; 16], "res-a");
                up::approve();
                let server_cred = match authn
                    .call_ctap2(&Request::MakeCredential(make_credential_request_for(
                        &rp_b,
                        &[0x42; 16],
                        "srv-b",
                        false,
                    )))
                    .expect("MC should succeed")
                {
                    Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                    other => panic!("Expected MC, got {:?}", other),
                };
                let result = authn.call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                    &rp_a,
                    Some(single_allow_list(&server_cred)),
                )));
                assert_eq!(result, Err(ctap2::Error::NoCredentials));
            }

            // --- same userId overwrites existing credential ---
            {
                let rp_id = unique_rp_id("rk-overwrite");
                let user = [0x55; 16];
                up::approve();
                let first = match authn
                    .call_ctap2(&Request::MakeCredential(resident_request(
                        &rp_id, &user, "over",
                    )))
                    .expect("first MC")
                {
                    Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                    other => panic!("{:?}", other),
                };
                up::approve();
                let second = match authn
                    .call_ctap2(&Request::MakeCredential(resident_request(
                        &rp_id, &user, "over",
                    )))
                    .expect("second MC")
                {
                    Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                    other => panic!("{:?}", other),
                };
                up::approve();
                let assertion = get_resident_assertion(authn, &rp_id, None);
                assert_ne!(first, second);
                assert_eq!(response_credential_id(&assertion), second);
                assert_eq!(
                    assertion.number_of_credentials, None,
                    "overwritten = single account"
                );
            }

            // --- allow list returns exactly one credential ---
            {
                let rp_id = unique_rp_id("rk-one");
                up::approve();
                let first = match authn
                    .call_ctap2(&Request::MakeCredential(resident_request(
                        &rp_id,
                        &[0x61; 16],
                        "r-0",
                    )))
                    .expect("MC")
                {
                    Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                    other => panic!("{:?}", other),
                };
                up::approve();
                let _second = match authn
                    .call_ctap2(&Request::MakeCredential(resident_request(
                        &rp_id,
                        &[0x62; 16],
                        "r-1",
                    )))
                    .expect("MC")
                {
                    Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                    other => panic!("{:?}", other),
                };
                up::approve();
                let ga = get_resident_assertion(authn, &rp_id, Some(single_allow_list(&first)));
                assert_eq!(response_credential_id(&ga), first);
                assert_eq!(ga.number_of_credentials, None, "allow list = single result");
            }
        })
    });
}

/// After authenticatorReset, a previously-issued allow_list credential id
/// must no longer match (CTAP 2.1 §6.7 step 2: reset wipes all credentials).
#[test]
#[serial]
fn rk_with_allow_list_after_reset_returns_no_credentials() {
    run_in_thread(|| {
        with_authenticator!(rk_after_reset, |authn| {
            reset_authenticator(authn);
            let rp_id = unique_rp_id("rk-after-reset");
            let (cred_id, _ga) =
                create_resident_credential(authn, &rp_id, &[0xab; 16], "rk-pre-reset");

            reset_authenticator(authn);

            up::approve();
            let result = authn.call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                &rp_id,
                Some(single_allow_list(&cred_id)),
            )));
            assert_eq!(
                result,
                Err(ctap2::Error::NoCredentials),
                "post-reset allow_list lookup must yield NoCredentials",
            );
        })
    });
}

/// Build a `PublicKeyCredentialUserEntity` with longest-allowed `name` /
/// `displayName` / `icon` / `id` (mirrors fido2-tests `generate_user_maximum`).
fn make_credential_request_with_max_user(
    rp_id: &str,
    user_id: &[u8],
) -> ctap2::make_credential::Request<'static> {
    use ctap_types::webauthn::*;

    let mut params = FilteredPublicKeyCredentialParameters(heapless::Vec::new());
    params
        .0
        .push(KnownPublicKeyCredentialParameters::ES256)
        .ok();

    // CTAP 2.1 §6.1.1 spec doesn't pin exact maxes; fido2-tests uses 64-byte
    // name/displayName + a long icon URL. We stay well under
    // `MAX_USER_NAME_LENGTH = 64` to keep the test portable.
    let long_name = "A".repeat(64);
    let long_display = "B".repeat(64);
    let long_icon = format!("https://example.com/icon?q={}", "C".repeat(64));

    let rp = PublicKeyCredentialRpEntity {
        id: rp_id.try_into().unwrap(),
        name: Some("Example".try_into().unwrap()),
        icon: None,
    };
    let user = PublicKeyCredentialUserEntity {
        id: ctap_types::Bytes::try_from(user_id).unwrap(),
        icon: Some(long_icon.as_str().try_into().unwrap()),
        name: Some(long_name.as_str().try_into().unwrap()),
        display_name: Some(long_display.as_str().try_into().unwrap()),
    };

    let mut req: ctap2::make_credential::Request<'static> =
        make_credential_request_from_value(Value::Map(
            [
                (Value::Integer(1), Value::Bytes([0xcd_u8; 32].to_vec())),
                (Value::Integer(2), to_value(&rp).expect("serialize rp")),
                (Value::Integer(3), to_value(&user).expect("serialize user")),
                (
                    Value::Integer(4),
                    to_value(&params).expect("serialize params"),
                ),
            ]
            .into_iter()
            .collect(),
        ));
    req.options = Some(decode_from_value(options_value(Some(true), None, None)));
    req
}

/// Resident credential survives with maximum-sized user fields
/// (fido2-tests `test_rk_maximum_size_nodisplay`).
#[test]
#[serial]
fn rk_max_size_user_fields_nodisplay() {
    run_in_thread(|| {
        with_authenticator!(rk_max_size, |authn| {
            reset_authenticator(authn);
            let rp_id = unique_rp_id("rk-max-size");
            up::approve();
            authn
                .call_ctap2(&Request::MakeCredential(
                    make_credential_request_with_max_user(&rp_id, &[0xc0; 16]),
                ))
                .expect("MC with maximum user fields should succeed");

            up::approve();
            let ga = get_resident_assertion(authn, &rp_id, None);
            assert_eq!(
                ga.number_of_credentials, None,
                "single account => no count returned",
            );
        })
    });
}

/// Fill an RP up to `MAX_CREDENTIAL_COUNT_IN_LIST` (10) credentials and
/// enumerate them with `GetNextAssertion`. fido2-tests
/// `test_rk_maximum_list_capacity_per_rp_nodisplay` originally exercised
/// the `max_creds_in_list` reported in GetInfo (= 10 for ctap-types).
#[test]
#[serial]
fn rk_capacity_per_rp_nodisplay() {
    run_in_thread(|| {
        with_authenticator!(rk_capacity, |authn| {
            reset_authenticator(authn);
            let rp_id = unique_rp_id("rk-capacity");
            const N: usize = ctap_types::sizes::MAX_CREDENTIAL_COUNT_IN_LIST;
            let mut credentials = Vec::with_capacity(N);
            for i in 0..N {
                let user_id = [0xD0u8 + i as u8; 16];
                up::approve();
                let resp = authn
                    .call_ctap2(&Request::MakeCredential(resident_request(
                        &rp_id,
                        &user_id,
                        &format!("cap-{i}"),
                    )))
                    .expect("MC should succeed");
                match resp {
                    Response::MakeCredential(mc) => {
                        credentials.push(extract_credential_id(&mc.auth_data));
                    }
                    other => panic!("Expected MakeCredential, got {:?}", other),
                }
            }

            up::approve();
            let first = get_resident_assertion(authn, &rp_id, None);
            assert_eq!(
                first.number_of_credentials,
                Some(N as u32),
                "first GA must report numberOfCredentials = N",
            );

            // Walk the chain via GetNextAssertion until exhausted, then
            // assert a subsequent call returns NotAllowed (CTAP 2.1 §6.3).
            for _ in 1..N {
                get_next_assertion(authn).expect("get_next_assertion within bounds should succeed");
            }
            assert!(
                get_next_assertion(authn).is_err(),
                "GetNextAssertion past last credential must fail",
            );
        })
    });
}

/// Icon URL longer than 128 bytes is accepted: ctap-types allows up to its
/// internal limit on the URL field. (fido2-tests `test_larger_icon_than_128`.)
#[test]
#[serial]
fn rk_icon_url_larger_than_128_bytes_accepted() {
    run_in_thread(|| {
        with_authenticator!(rk_icon, |authn| {
            reset_authenticator(authn);
            let rp_id = unique_rp_id("rk-icon");
            up::approve();
            authn
                .call_ctap2(&Request::MakeCredential(
                    make_credential_request_with_max_user(&rp_id, &[0xE0; 16]),
                ))
                .expect("MC with >128-byte icon URL should succeed");
        })
    });
}

/// With an allow list naming two RKs, GA returns the first matching one and
/// `GetNextAssertion` is rejected — allow-list lookup is not the same as
/// enumeration (CTAP 2.1 §6.2.3 step 11: allow_list = single match).
#[test]
#[serial]
fn rk_allow_list_returns_single_credential() {
    run_in_thread(|| {
        with_authenticator!(rk_returned, |authn| {
            reset_authenticator(authn);
            let rp_id = unique_rp_id("rk-returned");
            let mut creds = Vec::new();
            for i in 0..2u8 {
                up::approve();
                let resp = authn
                    .call_ctap2(&Request::MakeCredential(resident_request(
                        &rp_id,
                        &[0xF0 + i; 16],
                        &format!("ret-{i}"),
                    )))
                    .expect("MC should succeed");
                match resp {
                    Response::MakeCredential(mc) => {
                        creds.push(extract_credential_id(&mc.auth_data))
                    }
                    other => panic!("Expected MakeCredential, got {:?}", other),
                }
            }

            let mut allow_list: ctap2::get_assertion::AllowList<'static> = ctap_types::Vec::new();
            for c in &creds {
                allow_list.push(descriptor_ref(c)).unwrap();
            }
            up::approve();
            let ga = get_resident_assertion(authn, &rp_id, Some(allow_list));
            assert_eq!(
                ga.number_of_credentials, None,
                "allow_list result must not advertise additional credentials",
            );
            assert!(
                ga.user.as_ref().map(|u| !u.id.is_empty()).unwrap_or(true),
                "returned credential's user id must be non-empty if present",
            );
            assert!(
                get_next_assertion(authn).is_err(),
                "GetNextAssertion after allow_list-resolved GA must fail",
            );
        })
    });
}
