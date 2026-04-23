//! Table-driven parity port of legacy pytest GetAssertion request-validation cases.

use super::*;
use serde_cbor::Value;
use support::raw;

#[derive(Copy, Clone)]
enum ExpectedStatus {
    Exact(u8),
    OneOf(&'static [u8]),
}

struct RawGaCase {
    name: &'static str,
    request: fn() -> Value,
    expected: ExpectedStatus,
}

fn ga_command() -> u8 {
    0x02
}

fn raw_allow_list_entry(id: Value, key_type: Value) -> Value {
    raw::map([(raw::text("id"), id), (raw::text("type"), key_type)])
}

fn raw_ga_base() -> raw::CborMap {
    std::collections::BTreeMap::from([
        (raw::int_key(1), raw::text("example.com")),
        (raw::int_key(2), raw::bytes([0xcd; 32])),
        (
            raw::int_key(3),
            raw::array([raw_allow_list_entry(
                raw::bytes_vec(vec![0x01, 0x02, 0x03, 0x04]),
                raw::text("public-key"),
            )]),
        ),
    ])
}

fn raw_ga_value(edit: impl FnOnce(&mut raw::CborMap)) -> Value {
    let mut value = raw_ga_base();
    edit(&mut value);
    Value::Map(value)
}

fn assert_raw_ga_case(authn: &mut dyn TestAuthenticator, case: &RawGaCase) {
    let payload = raw::encode(&(case.request)());
    let (status, _response) = authn
        .call_ctap2_raw(ga_command(), &payload)
        .expect("raw GetAssertion transport failed");

    match case.expected {
        ExpectedStatus::Exact(expected) => {
            assert_eq!(status, expected, "case `{}`", case.name);
        }
        ExpectedStatus::OneOf(expected) => {
            assert!(
                expected.contains(&status),
                "case `{}`: expected one of {:02x?}, got 0x{status:02x}",
                case.name,
                expected
            );
        }
    }
}

const GA_REQUIRED_AND_TYPE_CASES: &[RawGaCase] = &[
    RawGaCase {
        name: "missing_rp",
        request: || {
            raw_ga_value(|m| {
                m.remove(&raw::int_key(1));
            })
        },
        expected: ExpectedStatus::Exact(0x14),
    },
    RawGaCase {
        name: "missing_client_data_hash",
        request: || {
            raw_ga_value(|m| {
                m.remove(&raw::int_key(2));
            })
        },
        expected: ExpectedStatus::Exact(0x14),
    },
    RawGaCase {
        name: "bad_rp",
        request: || {
            raw_ga_value(|m| {
                m.insert(
                    raw::int_key(1),
                    raw::map([(
                        raw::text("id"),
                        raw::map([(raw::text("type"), raw::text("wrong"))]),
                    )]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawGaCase {
        name: "bad_client_data_hash",
        request: || {
            raw_ga_value(|m| {
                m.insert(
                    raw::int_key(2),
                    raw::map([(raw::text("type"), raw::text("wrong"))]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawGaCase {
        name: "bad_allow_list",
        request: || {
            raw_ga_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::map([(raw::text("type"), raw::text("wrong"))]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawGaCase {
        name: "bad_allow_list_item",
        request: || {
            raw_ga_value(|m| {
                m.insert(raw::int_key(3), raw::array([raw::text("wrong")]));
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawGaCase {
        name: "allow_list_missing_field",
        request: || {
            raw_ga_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::array([
                        raw::map([(raw::text("id"), raw::bytes_vec(b"1234".to_vec()))]),
                        raw_allow_list_entry(
                            raw::bytes_vec(vec![0x01, 0x02, 0x03, 0x04]),
                            raw::text("public-key"),
                        ),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12, 0x14]),
    },
    RawGaCase {
        name: "allow_list_field_wrong_type",
        request: || {
            raw_ga_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::array([
                        raw_allow_list_entry(
                            raw::bytes_vec(b"1234".to_vec()),
                            raw::bytes_vec(b"public-key".to_vec()),
                        ),
                        raw_allow_list_entry(
                            raw::bytes_vec(vec![0x01, 0x02, 0x03, 0x04]),
                            raw::text("public-key"),
                        ),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawGaCase {
        name: "allow_list_id_wrong_type",
        request: || {
            raw_ga_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::array([
                        raw_allow_list_entry(Value::Integer(42), raw::text("public-key")),
                        raw_allow_list_entry(
                            raw::bytes_vec(vec![0x01, 0x02, 0x03, 0x04]),
                            raw::text("public-key"),
                        ),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawGaCase {
        name: "allow_list_missing_id",
        request: || {
            raw_ga_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::array([
                        raw::map([(raw::text("type"), raw::text("public-key"))]),
                        raw_allow_list_entry(
                            raw::bytes_vec(vec![0x01, 0x02, 0x03, 0x04]),
                            raw::text("public-key"),
                        ),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12, 0x14]),
    },
];

const GA_TOLERATED_CASES: &[RawGaCase] = &[
    RawGaCase {
        name: "unknown_option",
        request: || {
            raw_ga_value(|m| {
                m.insert(
                    raw::int_key(5),
                    raw::map([(raw::text("unknown"), Value::Bool(true))]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x00, 0x2E]),
    },
    RawGaCase {
        name: "allow_list_fake_item",
        request: || {
            raw_ga_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::array([
                        raw_allow_list_entry(raw::bytes_vec(b"1234".to_vec()), raw::text("rot13")),
                        raw_allow_list_entry(
                            raw::bytes_vec(vec![0x01, 0x02, 0x03, 0x04]),
                            raw::text("public-key"),
                        ),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x00, 0x2E]),
    },
];

#[test]
#[serial]
fn ga_raw_request_validation() {
    run_in_thread(|| {
        with_authenticator!(ga_raw_request_validation, |authn| {
            reset_authenticator(authn);
            for case in GA_REQUIRED_AND_TYPE_CASES {
                assert_raw_ga_case(authn, case);
            }
            for case in GA_TOLERATED_CASES {
                assert_raw_ga_case(authn, case);
            }
        })
    });
}
