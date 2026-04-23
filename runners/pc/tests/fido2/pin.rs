//! ClientPin and PIN-gated request coverage ported from the legacy pytest suite.

use super::*;
use serde_cbor::Value;
use support::pin::{self, PinSession};
use support::raw;

const PIN1: &str = "123456789A";
const PIN2: &str = "ABCDEF";

fn make_credential_with_pin(authn: &mut dyn TestAuthenticator, pin: &PinSession) -> Vec<u8> {
    let mut request = make_credential_request();
    request.pin_protocol = Some(pin.protocol().into());
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

fn get_assertion_with_pin(
    authn: &mut dyn TestAuthenticator,
    credential_id: &[u8],
    pin: Option<&PinSession>,
) -> ctap2::get_assertion::Response {
    let mut request = get_assertion_request(credential_id);
    if let Some(pin) = pin {
        request.pin_protocol = Some(pin.protocol().into());
        let pin_auth = pin.pin_auth_for_client_data_hash(request.client_data_hash.as_ref());
        request.pin_auth = Some(leak_bytes(pin_auth.to_vec()));
    }

    match authn
        .call_ctap2(&Request::GetAssertion(request))
        .expect("GetAssertion should succeed")
    {
        Response::GetAssertion(ga) => ga,
        other => panic!("Expected GetAssertion, got {:?}", other),
    }
}

struct PinSetupCase {
    name: &'static str,
    run: fn(&mut dyn TestAuthenticator) -> Result<(), ctap2::Error>,
    expected: ctap2::Error,
}

const PIN_SETUP_CASES: &[PinSetupCase] = &[
    PinSetupCase {
        name: "get_pin_token_without_pin",
        run: |authn| PinSession::try_get_pin_token(authn, PIN1).map(|_| ()),
        expected: ctap2::Error::PinNotSet,
    },
    PinSetupCase {
        name: "change_pin_without_pin",
        run: |authn| PinSession::try_change_pin(authn, PIN1, PIN2),
        expected: ctap2::Error::PinNotSet,
    },
    PinSetupCase {
        name: "set_pin_too_long",
        run: |authn| PinSession::try_set_pin(authn, &"A".repeat(64)),
        expected: ctap2::Error::PinPolicyViolation,
    },
];

#[test]
#[serial]
fn pin_setup_and_get_info() {
    run_isolated_in_sim("pin::pin_setup_and_get_info", || {
        run_in_thread(|| {
            with_authenticator!(pin_setup_and_get_info, |authn| {
                reset_authenticator(authn);
                for case in PIN_SETUP_CASES {
                    let actual = (case.run)(authn);
                    assert_eq!(actual, Err(case.expected), "case `{}`", case.name);
                }

                PinSession::set_pin(authn, PIN1);
                assert_eq!(
                    PinSession::try_set_pin(authn, "1234"),
                    Err(ctap2::Error::NotAllowed)
                );

                let info = match authn
                    .call_ctap2(&Request::GetInfo)
                    .expect("GetInfo should succeed")
                {
                    Response::GetInfo(info) => info,
                    other => panic!("Expected GetInfo, got {:?}", other),
                };

                assert_eq!(info.options.unwrap().client_pin, Some(true));
                assert_eq!(pin::get_retries(authn), 8);
                let _ = PinSession::get_pin_token(authn, PIN1);
            })
        });
    });
}

#[test]
#[serial]
fn pin_required_for_make_credential_but_optional_for_get_assertion() {
    run_isolated_in_sim(
        "pin::pin_required_for_make_credential_but_optional_for_get_assertion",
        || {
            run_in_thread(|| {
                with_authenticator!(pin_required_for_make_credential, |authn| {
                    reset_authenticator(authn);
                    PinSession::set_pin(authn, PIN1);
                    let pin = PinSession::get_pin_token(authn, PIN1);
                    let credential_id = make_credential_with_pin(authn, &pin);

                    let assertion = get_assertion_with_pin(authn, &credential_id, None);
                    assert_eq!(
                        assertion.auth_data[32] & (1 << 2),
                        0,
                        "UV should not be set without pinAuth"
                    );

                    assert_eq!(
                        authn.call_ctap2(&Request::MakeCredential(make_credential_request_for(
                            "pin-required.example",
                            &[0x77; 16],
                            "noraw",
                            true,
                        ))),
                        Err(ctap2::Error::PinRequired),
                    );
                })
            });
        },
    );
}

#[derive(Copy, Clone)]
struct EmptyPinAuthCase {
    name: &'static str,
    command: u8,
    request: fn(&[u8]) -> Value,
    expected: ctap2::Error,
}

fn raw_empty_pin_mc_request(_credential_id: &[u8]) -> Value {
    raw::map([
        (raw::int_key(1), raw::bytes([0xcd; 32])),
        (
            raw::int_key(2),
            raw::map([
                (raw::text("id"), raw::text("example.com")),
                (raw::text("name"), raw::text("Example")),
            ]),
        ),
        (
            raw::int_key(3),
            raw::map([
                (raw::text("id"), raw::bytes([0x01; 16])),
                (raw::text("name"), raw::text("testuser")),
                (raw::text("displayName"), raw::text("Test User")),
            ]),
        ),
        (
            raw::int_key(4),
            raw::array([raw::map([
                (raw::text("type"), raw::text("public-key")),
                (raw::text("alg"), Value::Integer(-7)),
            ])]),
        ),
        (raw::int_key(8), raw::bytes_vec(Vec::new())),
        (raw::int_key(9), Value::Integer(1)),
    ])
}

fn raw_empty_pin_ga_request(credential_id: &[u8]) -> Value {
    raw::map([
        (raw::int_key(1), raw::text("example.com")),
        (raw::int_key(2), raw::bytes([0xcd; 32])),
        (
            raw::int_key(3),
            raw::array([raw::map([
                (raw::text("type"), raw::text("public-key")),
                (raw::text("id"), raw::bytes_vec(credential_id.to_vec())),
            ])]),
        ),
        (raw::int_key(6), raw::bytes_vec(Vec::new())),
        (raw::int_key(7), Value::Integer(1)),
    ])
}

const EMPTY_PIN_AUTH_CASES: &[EmptyPinAuthCase] = &[
    EmptyPinAuthCase {
        name: "make_credential",
        command: 0x01,
        request: raw_empty_pin_mc_request,
        expected: ctap2::Error::PinAuthInvalid,
    },
    EmptyPinAuthCase {
        name: "get_assertion",
        command: 0x02,
        request: raw_empty_pin_ga_request,
        expected: ctap2::Error::PinAuthInvalid,
    },
];

#[test]
#[serial]
fn pin_zero_length_pin_auth_is_rejected() {
    run_isolated_in_sim("pin::pin_zero_length_pin_auth_is_rejected", || {
        run_in_thread(|| {
            with_authenticator!(pin_zero_length_pin_auth, |authn| {
                reset_authenticator(authn);
                PinSession::set_pin(authn, PIN1);
                let pin = PinSession::get_pin_token(authn, PIN1);
                let credential_id = make_credential_with_pin(authn, &pin);

                for case in EMPTY_PIN_AUTH_CASES {
                    let payload = raw::encode(&(case.request)(&credential_id));
                    let (status, _response) = authn
                        .call_ctap2_raw(case.command, &payload)
                        .expect("raw pinAuth transport should succeed");
                    assert_eq!(
                        transport::error_from_byte(status),
                        case.expected,
                        "case `{}`",
                        case.name
                    );
                }
            })
        });
    });
}

#[test]
#[serial]
fn pin_change_updates_active_pin() {
    run_isolated_in_sim("pin::pin_change_updates_active_pin", || {
        run_in_thread(|| {
            with_authenticator!(pin_change_updates_active_pin, |authn| {
                reset_authenticator(authn);
                PinSession::set_pin(authn, PIN1);
                let first_pin = PinSession::get_pin_token(authn, PIN1);
                let first_credential = make_credential_with_pin(authn, &first_pin);

                PinSession::change_pin(authn, PIN1, PIN2);

                assert_eq!(
                    PinSession::try_get_pin_token(authn, PIN1).map(|_| ()),
                    Err(ctap2::Error::PinInvalid),
                );

                let second_pin = PinSession::get_pin_token(authn, PIN2);
                let second_credential = make_credential_with_pin(authn, &second_pin);
                let assertion =
                    get_assertion_with_pin(authn, &second_credential, Some(&second_pin));

                assert_ne!(first_credential, second_credential);
                assert_ne!(
                    assertion.auth_data[32] & (1 << 2),
                    0,
                    "UV should be set with pinAuth"
                );
            })
        });
    });
}

struct PinAttemptCase {
    expected_error: ctap2::Error,
    retries_after: u8,
}

const PIN_ATTEMPT_CASES: &[PinAttemptCase] = &[
    PinAttemptCase {
        expected_error: ctap2::Error::PinInvalid,
        retries_after: 7,
    },
    PinAttemptCase {
        expected_error: ctap2::Error::PinInvalid,
        retries_after: 6,
    },
    PinAttemptCase {
        expected_error: ctap2::Error::PinAuthBlocked,
        retries_after: 5,
    },
    PinAttemptCase {
        expected_error: ctap2::Error::PinAuthBlocked,
        retries_after: 5,
    },
];

#[test]
#[serial]
fn pin_attempts_escalate_and_decrement_retries() {
    run_isolated_in_sim("pin::pin_attempts_escalate_and_decrement_retries", || {
        run_in_thread(|| {
            with_authenticator!(pin_attempts_escalate, |authn| {
                reset_authenticator(authn);
                PinSession::set_pin(authn, PIN1);

                for case in PIN_ATTEMPT_CASES {
                    let actual = PinSession::try_get_pin_token(authn, "wrong-pin").map(|_| ());
                    assert_eq!(actual, Err(case.expected_error));
                    assert_eq!(pin::get_retries(authn), case.retries_after);
                }
            })
        });
    });
}
