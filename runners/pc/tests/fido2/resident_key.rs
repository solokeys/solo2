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
