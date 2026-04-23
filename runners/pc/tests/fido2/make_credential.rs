//! MakeCredential tests.

use super::*;
use serde_cbor::Value;
use support::raw;

/// Core MC tests: basic success, algorithms, exclude lists.
#[test]
#[serial]
fn mc_group_basic() {
    run_in_thread(|| {
        with_authenticator!(mc_group_basic, |authn| {
            reset_authenticator(authn);

            // --- basic MC ---
            up::approve();
            let resp = authn
                .call_ctap2(&Request::MakeCredential(make_credential_request()))
                .expect("basic MC should succeed");
            match &resp {
                Response::MakeCredential(mc) => {
                    assert!(mc.auth_data.len() >= 77);
                    let flags = mc.auth_data[32];
                    assert!(flags & 0x01 != 0, "UP flag");
                    assert!(flags & 0x40 != 0, "AT flag");
                }
                other => panic!("Expected MakeCredential, got {:?}", other),
            }

            // --- EdDSA ---
            let mut req = make_credential_request();
            req.pub_key_cred_params = pkcp_for(&[-8]);
            up::approve();
            authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("EdDSA MC should succeed");

            // --- unsupported algorithm errors ---
            let mut req = make_credential_request();
            req.pub_key_cred_params = pkcp_for(&[]);
            assert!(
                authn.call_ctap2(&Request::MakeCredential(req)).is_err(),
                "empty params should fail"
            );

            let mut req = make_credential_request();
            req.pub_key_cred_params = pkcp_for(&[-257]);
            assert!(
                authn.call_ctap2(&Request::MakeCredential(req)).is_err(),
                "RS256-only should fail"
            );

            // --- exclude list blocks existing credential ---
            up::approve();
            let cred_id = make_credential(authn);
            let mut req = make_credential_request();
            let mut list = ctap_types::Vec::new();
            list.push(descriptor_ref(&cred_id)).unwrap();
            req.exclude_list = Some(list);
            up::approve();
            assert!(
                authn.call_ctap2(&Request::MakeCredential(req)).is_err(),
                "excluded cred should fail"
            );

            // --- exclude list with unknown type is tolerated ---
            let mut req = make_credential_request();
            let mut list = ctap_types::Vec::new();
            list.push(descriptor_ref_typed(&[0xde, 0xad], "weird-type"))
                .unwrap();
            req.exclude_list = Some(list);
            up::approve();
            authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("unknown type should be ignored");
        })
    });
}

// --- Raw CBOR request validation (table-driven) ---

#[derive(Copy, Clone)]
enum ExpectedStatus {
    Exact(u8),
    OneOf(&'static [u8]),
}

struct RawMcCase {
    name: &'static str,
    request: fn() -> Value,
    expected: ExpectedStatus,
}

fn mc_command() -> u8 {
    0x01
}

fn raw_mc_base() -> raw::CborMap {
    std::collections::BTreeMap::from([
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
    ])
}

fn raw_mc_value(edit: impl FnOnce(&mut raw::CborMap)) -> Value {
    let mut value = raw_mc_base();
    edit(&mut value);
    Value::Map(value)
}

fn raw_mc_payload(value: Value) -> Vec<u8> {
    raw::encode(&value)
}

fn assert_raw_mc_case(authn: &mut dyn TestAuthenticator, case: &RawMcCase) {
    let payload = raw_mc_payload((case.request)());
    let (status, _response) = authn
        .call_ctap2_raw(mc_command(), &payload)
        .expect("raw MakeCredential transport failed");
    match case.expected {
        ExpectedStatus::Exact(expected) => assert_eq!(status, expected, "case `{}`", case.name),
        ExpectedStatus::OneOf(expected) => assert!(
            expected.contains(&status),
            "case `{}`: expected one of {:02x?}, got 0x{status:02x}",
            case.name,
            expected
        ),
    }
}

const MC_REQUIRED_FIELD_CASES: &[RawMcCase] = &[
    RawMcCase {
        name: "missing_cdh",
        request: || {
            raw_mc_value(|m| {
                m.remove(&raw::int_key(1));
            })
        },
        expected: ExpectedStatus::Exact(0x14),
    },
    RawMcCase {
        name: "missing_rp",
        request: || {
            raw_mc_value(|m| {
                m.remove(&raw::int_key(2));
            })
        },
        expected: ExpectedStatus::Exact(0x14),
    },
    RawMcCase {
        name: "missing_user",
        request: || {
            raw_mc_value(|m| {
                m.remove(&raw::int_key(3));
            })
        },
        expected: ExpectedStatus::Exact(0x14),
    },
    RawMcCase {
        name: "missing_params",
        request: || {
            raw_mc_value(|m| {
                m.remove(&raw::int_key(4));
            })
        },
        expected: ExpectedStatus::Exact(0x14),
    },
];

const MC_BAD_TYPE_CASES: &[RawMcCase] = &[
    RawMcCase {
        name: "bad_type_cdh",
        request: || {
            raw_mc_value(|m| {
                m.insert(raw::int_key(1), Value::Integer(5));
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_rp",
        request: || {
            raw_mc_value(|m| {
                m.insert(raw::int_key(2), raw::bytes_vec(b"rp".to_vec()));
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_user",
        request: || {
            raw_mc_value(|m| {
                m.insert(raw::int_key(3), raw::bytes_vec(b"u".to_vec()));
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_params",
        request: || {
            raw_mc_value(|m| {
                m.insert(raw::int_key(4), raw::bytes_vec(b"p".to_vec()));
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_exclude",
        request: || {
            raw_mc_value(|m| {
                m.insert(raw::int_key(5), Value::Integer(8));
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_ext",
        request: || {
            raw_mc_value(|m| {
                m.insert(raw::int_key(6), Value::Integer(8));
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_options",
        request: || {
            raw_mc_value(|m| {
                m.insert(raw::int_key(7), Value::Integer(8));
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_rp_name",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(2),
                    raw::map([
                        (raw::text("id"), raw::text("t.org")),
                        (raw::text("name"), Value::Integer(8)),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_user_name",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::map([
                        (raw::text("id"), raw::bytes_vec(b"uid".to_vec())),
                        (raw::text("name"), Value::Integer(8)),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_user_display",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::map([
                        (raw::text("id"), raw::bytes_vec(b"uid".to_vec())),
                        (raw::text("name"), raw::text("n")),
                        (raw::text("displayName"), Value::Integer(8)),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        name: "bad_type_user_icon",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::map([
                        (raw::text("id"), raw::bytes_vec(b"uid".to_vec())),
                        (raw::text("name"), raw::text("n")),
                        (raw::text("icon"), Value::Integer(8)),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x00, 0x02, 0x11, 0x12]),
    },
];

#[test]
#[serial]
fn mc_raw_request_validation() {
    run_in_thread(|| {
        with_authenticator!(mc_raw, |authn| {
            reset_authenticator(authn);
            for case in MC_REQUIRED_FIELD_CASES {
                assert_raw_mc_case(authn, case);
            }
            for case in MC_BAD_TYPE_CASES {
                assert_raw_mc_case(authn, case);
            }
        })
    });
}
