//! Credential management coverage ported from the legacy pytest suite.

use super::*;
use serde_cbor::Value;
use sha2::{Digest, Sha256};
use support::cred_mgmt::{
    self, credential_management_request, raw_credential_management, CredentialManagementSession,
};
use support::pin::PinSession;

const TEST_PIN: &str = "123456";

fn rp_id_hash(rp_id: &str) -> [u8; 32] {
    Sha256::digest(rp_id.as_bytes()).into()
}

fn create_resident_credential_with_pin(
    authn: &mut dyn TestAuthenticator,
    pin: &PinSession,
    rp_id: &str,
    user_id: &[u8],
    user_name: &str,
) -> Vec<u8> {
    let mut request = make_credential_request_for(rp_id, user_id, user_name, true);
    request.pin_protocol = Some(1);
    let pin_auth = pin.pin_auth_for_client_data_hash(request.client_data_hash.as_ref());
    request.pin_auth = Some(leak_bytes(pin_auth.to_vec()));

    match authn
        .call_ctap2(&Request::MakeCredential(request))
        .expect("MakeCredential with PIN should succeed")
    {
        Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
        other => panic!("Expected MakeCredential, got {:?}", other),
    }
}

fn get_resident_assertion_local(
    authn: &mut dyn TestAuthenticator,
    rp_id: &str,
) -> ctap2::get_assertion::Response {
    match authn
        .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
            rp_id, None,
        )))
        .expect("resident GetAssertion should succeed")
    {
        Response::GetAssertion(ga) => ga,
        other => panic!("Expected GetAssertion, got {:?}", other),
    }
}

fn assertion_credential_id(assertion: &ctap2::get_assertion::Response) -> Vec<u8> {
    assertion.credential.id.as_slice().to_vec()
}

fn provision_cred_mgmt_fixture(
    authn: &mut dyn TestAuthenticator,
) -> CredentialManagementSession<'_> {
    reset_authenticator(authn);
    PinSession::set_pin(authn, TEST_PIN);
    let pin = PinSession::get_pin_token_with_cred_mgmt(authn, TEST_PIN);
    create_resident_credential_with_pin(authn, &pin, "ssh:", &[0x10; 16], "ssh-user");
    create_resident_credential_with_pin(authn, &pin, "xakcop.com", &[0x20; 16], "xakcop-user");
    CredentialManagementSession::new(authn, pin)
}

fn enumerate_rps(session: &mut CredentialManagementSession<'_>) -> Vec<serde_cbor::Value> {
    let first = session.enumerate_rps_begin();
    let total = cred_mgmt::as_u64(cred_mgmt::map_get(&first, 5)) as usize;
    let mut rps = vec![first];
    for _ in 1..total {
        rps.push(
            session
                .enumerate_rps_next()
                .expect("EnumerateRpsGetNextRp should succeed"),
        );
    }
    rps
}

fn enumerate_creds(
    session: &mut CredentialManagementSession<'_>,
    rp_id: &str,
) -> Vec<serde_cbor::Value> {
    let first = session.enumerate_creds_begin(&rp_id_hash(rp_id));
    let total = cred_mgmt::as_u64(cred_mgmt::map_get(&first, 9)) as usize;
    let mut creds = vec![first];
    for _ in 1..total {
        creds.push(
            session
                .enumerate_creds_next()
                .expect("EnumerateCredentialsGetNextCredential should succeed"),
        );
    }
    creds
}

fn enumerate_tree(
    session: &mut CredentialManagementSession<'_>,
) -> std::collections::BTreeMap<String, usize> {
    let mut tree = std::collections::BTreeMap::new();
    for rp in enumerate_rps(session) {
        let rp_id = cred_mgmt::as_text(cred_mgmt::map_get_text(cred_mgmt::map_get(&rp, 3), "id"));
        tree.insert(rp_id.to_string(), enumerate_creds(session, rp_id).len());
    }
    tree
}

fn create_resident_credentials(
    authn: &mut dyn TestAuthenticator,
    pin: &PinSession,
    rp_id: &str,
    count: usize,
) -> Vec<Vec<u8>> {
    (0..count)
        .map(|index| {
            let user_id = [
                count as u8,
                index as u8,
                0x44,
                0x55,
                0x66,
                0x77,
                0x88,
                0x99,
                0xaa,
                0xbb,
                0xcc,
                0xdd,
                0xee,
                0xf0,
                0x01,
                0x02,
            ];
            create_resident_credential_with_pin(
                authn,
                pin,
                rp_id,
                &user_id,
                &format!("{rp_id}-user-{index}"),
            )
        })
        .collect()
}

fn assert_enumeration_matches(
    session: &mut CredentialManagementSession<'_>,
    expected: &[(&str, usize)],
) {
    let expected_map = expected
        .iter()
        .map(|(rp_id, count)| ((*rp_id).to_string(), *count))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(enumerate_tree(session), expected_map);
}

#[derive(Copy, Clone)]
enum EnumerationStyle {
    BreadthFirst,
    Interleaved,
}

fn assert_enumeration_style(
    session: &mut CredentialManagementSession<'_>,
    expected: &[(&str, usize)],
    style: EnumerationStyle,
) {
    match style {
        EnumerationStyle::BreadthFirst => assert_enumeration_matches(session, expected),
        EnumerationStyle::Interleaved => {
            let first_rp = session.enumerate_rps_begin();
            let total_rps = cred_mgmt::as_u64(cred_mgmt::map_get(&first_rp, 5)) as usize;
            assert_eq!(total_rps, expected.len());

            let mut seen = std::collections::BTreeMap::new();
            let mut current = first_rp;

            for index in 0..total_rps {
                let rp = cred_mgmt::map_get(&current, 3);
                let rp_id = cred_mgmt::as_text(cred_mgmt::map_get_text(rp, "id")).to_string();
                let first_cred = session.enumerate_creds_begin(&rp_id_hash(&rp_id));
                let total_creds = cred_mgmt::as_u64(cred_mgmt::map_get(&first_cred, 9)) as usize;
                for _ in 1..total_creds {
                    let _ = session
                        .enumerate_creds_next()
                        .expect("EnumerateCredentialsGetNextCredential should succeed");
                }
                seen.insert(rp_id, total_creds);

                if index + 1 < total_rps {
                    current = session
                        .enumerate_rps_next()
                        .expect("EnumerateRpsGetNextRp should succeed");
                }
            }

            let expected_map = expected
                .iter()
                .map(|(rp_id, count)| ((*rp_id).to_string(), *count))
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(seen, expected_map);
        }
    }
}

fn wrong_pin_auth_request(
    pin: &PinSession,
    sub_command: ctap2::credential_management::Subcommand,
    params: Option<Value>,
) -> Value {
    let mut pin_auth = pin.pin_auth_for_credential_management(sub_command, params.as_ref());
    pin_auth[0] ^= 0x80;
    credential_management_request(
        sub_command,
        params,
        Some(pin.protocol()),
        Some(pin_auth.as_slice()),
    )
}

#[test]
#[serial]
fn cred_mgmt_metadata() {
    run_isolated_in_sim("credential_management::cred_mgmt_metadata", || {
        run_in_thread(|| {
            with_authenticator!(cred_mgmt_metadata, |authn| {
                let mut session = provision_cred_mgmt_fixture(authn);
                let metadata = session.get_metadata();
                assert_eq!(cred_mgmt::as_u64(cred_mgmt::map_get(&metadata, 1)), 2);
                assert!(cred_mgmt::as_u64(cred_mgmt::map_get(&metadata, 2)) >= 1);
            })
        });
    });
}

#[test]
#[serial]
fn cred_mgmt_enumerates_rps_and_credentials() {
    run_isolated_in_sim(
        "credential_management::cred_mgmt_enumerates_rps_and_credentials",
        || {
            run_in_thread(|| {
                with_authenticator!(cred_mgmt_enumeration, |authn| {
                    let mut session = provision_cred_mgmt_fixture(authn);
                    let rps = enumerate_rps(&mut session);
                    assert_eq!(rps.len(), 2);

                    let mut rp_ids = rps
                        .iter()
                        .map(|rp| {
                            cred_mgmt::as_text(cred_mgmt::map_get_text(
                                cred_mgmt::map_get(rp, 3),
                                "id",
                            ))
                            .to_string()
                        })
                        .collect::<Vec<_>>();
                    rp_ids.sort();
                    assert_eq!(rp_ids, vec!["ssh:".to_string(), "xakcop.com".to_string()]);

                    for rp_id in &rp_ids {
                        let creds = enumerate_creds(&mut session, rp_id);
                        assert_eq!(creds.len(), 1, "expected one credential for {rp_id}");
                        assert!(
                            cred_mgmt::map_get_optional(&creds[0], 6).is_some(),
                            "user should be present"
                        );
                        assert!(
                            cred_mgmt::map_get_optional(&creds[0], 7).is_some(),
                            "credential id should be present"
                        );
                        assert!(
                            cred_mgmt::map_get_optional(&creds[0], 8).is_some(),
                            "public key should be present"
                        );
                        assert!(
                            cred_mgmt::map_get_optional(&creds[0], 10).is_some(),
                            "credProtect should be present"
                        );
                    }
                })
            });
        },
    );
}

#[derive(Copy, Clone)]
enum ContinuationKind {
    RpNextWithoutBegin,
    CredNextWithoutBegin,
}

const CONTINUATION_CASES: &[ContinuationKind] = &[
    ContinuationKind::RpNextWithoutBegin,
    ContinuationKind::CredNextWithoutBegin,
];

#[test]
#[serial]
fn cred_mgmt_rejects_invalid_continuations() {
    run_isolated_in_sim(
        "credential_management::cred_mgmt_rejects_invalid_continuations",
        || {
            run_in_thread(|| {
                with_authenticator!(cred_mgmt_invalid_continuations, |authn| {
                    let mut session = provision_cred_mgmt_fixture(authn);
                    for case in CONTINUATION_CASES {
                        let result = match case {
                            ContinuationKind::RpNextWithoutBegin => {
                                let _ = session.enumerate_creds_begin(&rp_id_hash("ssh:"));
                                session.enumerate_rps_next()
                            }
                            ContinuationKind::CredNextWithoutBegin => {
                                let _ = session.enumerate_rps_begin();
                                session.enumerate_creds_next()
                            }
                        };
                        assert_eq!(result, Err(ctap2::Error::NotAllowed));
                    }
                })
            });
        },
    );
}

#[test]
#[serial]
fn cred_mgmt_delete_removes_credential() {
    run_isolated_in_sim(
        "credential_management::cred_mgmt_delete_removes_credential",
        || {
            run_in_thread(|| {
                with_authenticator!(cred_mgmt_delete, |authn| {
                    reset_authenticator(authn);
                    PinSession::set_pin(authn, TEST_PIN);
                    let pin = PinSession::get_pin_token_with_cred_mgmt(authn, TEST_PIN);
                    let rp_id = "example-3.com";
                    let survivor_a = create_resident_credential_with_pin(
                        authn,
                        &pin,
                        rp_id,
                        &[0x30; 16],
                        "keep-a",
                    );
                    let removed = create_resident_credential_with_pin(
                        authn,
                        &pin,
                        rp_id,
                        &[0x31; 16],
                        "remove-me",
                    );
                    let survivor_b = create_resident_credential_with_pin(
                        authn,
                        &pin,
                        rp_id,
                        &[0x32; 16],
                        "keep-b",
                    );

                    let mut session = CredentialManagementSession::new(authn, pin);
                    let creds = enumerate_creds(&mut session, rp_id);
                    assert_eq!(creds.len(), 3);

                    let removed_credential = creds
                        .iter()
                        .map(|cred| {
                            cred_mgmt::as_bytes(cred_mgmt::map_get_text(
                                cred_mgmt::map_get(cred, 7),
                                "id",
                            ))
                        })
                        .find(|id| *id == removed.as_slice())
                        .expect("credential selected for deletion should enumerate")
                        .to_vec();

                    let _ = session.delete_credential(&removed_credential);

                    let remaining = enumerate_creds(&mut session, rp_id);
                    assert_eq!(remaining.len(), 2);
                    let remaining_ids = remaining
                        .iter()
                        .map(|cred| {
                            cred_mgmt::as_bytes(cred_mgmt::map_get_text(
                                cred_mgmt::map_get(cred, 7),
                                "id",
                            ))
                            .to_vec()
                        })
                        .collect::<Vec<_>>();

                    assert!(remaining_ids.iter().any(|id| id == &survivor_a));
                    assert!(remaining_ids.iter().any(|id| id == &survivor_b));
                    assert!(remaining_ids.iter().all(|id| id != &removed));

                    let assertion = get_resident_assertion_local(authn, rp_id);
                    let returned_id = assertion_credential_id(&assertion);
                    assert_ne!(returned_id, removed);
                    assert!(returned_id == survivor_a || returned_id == survivor_b);
                })
            });
        },
    );
}

#[test]
#[serial]
fn cred_mgmt_multiple_rps_and_credential_counts() {
    run_isolated_in_sim(
        "credential_management::cred_mgmt_multiple_rps_and_credential_counts",
        || {
            run_in_thread(|| {
                with_authenticator!(cred_mgmt_multi_rp, |authn| {
                    let mut session = provision_cred_mgmt_fixture(authn);
                    assert_enumeration_matches(&mut session, &[("ssh:", 1), ("xakcop.com", 1)]);

                    let pin = PinSession::get_pin_token_with_cred_mgmt(authn, TEST_PIN);
                    let cases = [
                        ("new-example-1.com", 3),
                        ("new-example-2.com", 3),
                        ("new-example-3.com", 3),
                    ];

                    for (rp_id, count) in cases {
                        let _ = create_resident_credentials(authn, &pin, rp_id, count);
                    }

                    let mut session = CredentialManagementSession::new(authn, pin);
                    assert_enumeration_matches(
                        &mut session,
                        &[
                            ("new-example-1.com", 3),
                            ("new-example-2.com", 3),
                            ("new-example-3.com", 3),
                            ("ssh:", 1),
                            ("xakcop.com", 1),
                        ],
                    );
                })
            });
        },
    );
}

#[test]
#[serial]
fn cred_mgmt_enumeration_remains_consistent_after_updates() {
    run_isolated_in_sim(
        "credential_management::cred_mgmt_enumeration_remains_consistent_after_updates",
        || {
            run_in_thread(|| {
                with_authenticator!(cred_mgmt_multi_enumeration, |authn| {
                    let mut session = provision_cred_mgmt_fixture(authn);
                    let mut expected = vec![("ssh:", 1), ("xakcop.com", 1)];

                    for style in [
                        EnumerationStyle::BreadthFirst,
                        EnumerationStyle::Interleaved,
                    ] {
                        assert_enumeration_style(&mut session, &expected, style);
                    }

                    let pin = PinSession::get_pin_token_with_cred_mgmt(authn, TEST_PIN);
                    for (rp_id, count) in [
                        ("example-2.com", 2),
                        ("example-1.com", 1),
                        ("example-5.com", 5),
                    ] {
                        let _ = create_resident_credentials(authn, &pin, rp_id, count);
                        expected.push((rp_id, count));
                    }

                    let mut session = CredentialManagementSession::new(authn, pin);
                    for style in [
                        EnumerationStyle::BreadthFirst,
                        EnumerationStyle::Interleaved,
                    ] {
                        assert_enumeration_style(&mut session, &expected, style);
                        let _ = session.get_metadata();
                        assert_enumeration_style(&mut session, &expected, style);
                    }
                })
            });
        },
    );
}

#[derive(Copy, Clone)]
enum WrongPinAuthCase {
    Metadata,
    EnumerateRpsBegin,
    EnumerateCredentialsBegin,
}

impl WrongPinAuthCase {
    fn request(self, pin: &PinSession) -> Value {
        match self {
            WrongPinAuthCase::Metadata => wrong_pin_auth_request(
                pin,
                ctap2::credential_management::Subcommand::GetCredsMetadata,
                None,
            ),
            WrongPinAuthCase::EnumerateRpsBegin => wrong_pin_auth_request(
                pin,
                ctap2::credential_management::Subcommand::EnumerateRpsBegin,
                None,
            ),
            WrongPinAuthCase::EnumerateCredentialsBegin => wrong_pin_auth_request(
                pin,
                ctap2::credential_management::Subcommand::EnumerateCredentialsBegin,
                Some(Value::Map(
                    [(Value::Integer(1), Value::Bytes(rp_id_hash("ssh:").to_vec()))]
                        .into_iter()
                        .collect(),
                )),
            ),
        }
    }
}

fn assert_wrong_pin_auth_escalates(test_name: &'static str, case: WrongPinAuthCase) {
    run_isolated_in_sim(test_name, move || {
        run_in_thread(move || {
            with_authenticator!(cred_mgmt_wrong_pin_auth, |authn| {
                let _ = provision_cred_mgmt_fixture(authn);
                let pin = PinSession::get_pin_token_with_cred_mgmt(authn, TEST_PIN);

                for expected in [
                    ctap2::Error::PinAuthInvalid,
                    ctap2::Error::PinAuthInvalid,
                    ctap2::Error::PinAuthBlocked,
                ] {
                    let actual = raw_credential_management(authn, &case.request(&pin));
                    assert_eq!(actual, Err(expected));
                }
            })
        });
    });
}

#[test]
#[serial]
fn cred_mgmt_metadata_wrong_pin_auth_escalates() {
    assert_wrong_pin_auth_escalates(
        "credential_management::cred_mgmt_metadata_wrong_pin_auth_escalates",
        WrongPinAuthCase::Metadata,
    );
}

#[test]
#[serial]
fn cred_mgmt_enumerate_rps_wrong_pin_auth_escalates() {
    assert_wrong_pin_auth_escalates(
        "credential_management::cred_mgmt_enumerate_rps_wrong_pin_auth_escalates",
        WrongPinAuthCase::EnumerateRpsBegin,
    );
}

#[test]
#[serial]
fn cred_mgmt_enumerate_creds_wrong_pin_auth_escalates() {
    assert_wrong_pin_auth_escalates(
        "credential_management::cred_mgmt_enumerate_creds_wrong_pin_auth_escalates",
        WrongPinAuthCase::EnumerateCredentialsBegin,
    );
}
