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
                    // CTAP 2.1 §6.1.4: auth_data is rpIdHash(32) + flags(1) +
                    // signCount(4) + AT-data; minimum reasonable length with
                    // attested credential data is 77 bytes (32+1+4+aaguid(16)
                    // +credIdLen(2)+min credId + min COSE_Key).
                    assert!(mc.auth_data.len() >= 77, "auth_data too short");
                    let flags = mc.auth_data[32];
                    assert!(flags & 0x01 != 0, "UP flag");
                    assert!(flags & 0x40 != 0, "AT flag");
                    // CTAP 2.1 §6.1.4 step 16: attestation format must be a
                    // registered fmt. fido-authenticator returns "packed".
                    assert_eq!(
                        mc.fmt,
                        ctap2::AttestationStatementFormat::Packed,
                        "attestation format should be packed",
                    );
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

/// Acceptable CTAP2 status codes when a malformed member item (a bad
/// `pubKeyCredParams` entry or `excludeList` descriptor) must be rejected.
///
/// The exact sub-error is implementation-defined: ctap-types maps a missing
/// required field to `MissingParameter` (0x14, via `SerdeMissingField`), a
/// bad bool to `CborUnexpectedType` (0x11), and any other deserialization
/// failure to `InvalidCbor` (0x12); `InvalidParameter` (0x02) is also a valid
/// rejection. fido-authenticator returns 0x14/0x12 here depending on whether
/// the offending field was absent or merely the wrong type.
const MALFORMED_ITEM_STATUSES: &[u8] = &[0x02, 0x11, 0x12, 0x14];

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

// =============================================================================
// Deepening: cases ported from the FIDO CTAP2.3 conformance module
// (Authr-MakeCred-Req-2..6, Resp-1) that the table-driven cases above did not
// already cover. Each appended fn maps to one or more `it(`...`)` cases.
//
// Helpers reused from above: raw_mc_value, raw_mc_payload, mc_command,
// assert_raw_mc_case, RawMcCase, ExpectedStatus.
// =============================================================================

// --- Authr-MakeCred-Req-2 F-1 / Req-3 F-1: entity field types not yet covered ---
//
// The existing MC_BAD_TYPE_CASES table covers rp.name, user.name,
// user.displayName, user.icon bad types, but NOT rp.id (must be TEXT) nor
// user.id (must be BYTE STRING). Add those two.
const MC_ENTITY_BAD_TYPE_CASES: &[RawMcCase] = &[
    RawMcCase {
        // Req-2 F-1: rp.id is NOT of type TEXT.
        name: "bad_type_rp_id",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(2),
                    raw::map([
                        (raw::text("id"), Value::Integer(8)),
                        (raw::text("name"), raw::text("Example")),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
    RawMcCase {
        // Req-3 F-1: user.id is NOT of type BYTE ARRAY.
        name: "bad_type_user_id",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(3),
                    raw::map([
                        (raw::text("id"), raw::text("not-bytes")),
                        (raw::text("name"), raw::text("testuser")),
                        (raw::text("displayName"), raw::text("Test User")),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(&[0x02, 0x11, 0x12]),
    },
];

#[test]
#[serial]
fn mc_entity_field_bad_types() {
    run_in_thread(|| {
        with_authenticator!(mc_entity_field_bad_types, |authn| {
            reset_authenticator(authn);
            for case in MC_ENTITY_BAD_TYPE_CASES {
                assert_raw_mc_case(authn, case);
            }
        })
    });
}

// --- Authr-MakeCred-Req-4 F-1..F-5: malformed PublicKeyCredentialParameters items ---
//
// The existing table only exercises wholesale-bad pubKeyCredParams (not an
// array). These cover a malformed *item* inside the array. fido-authenticator
// deserializes pubKeyCredParams into a filtered list of known {type,alg}; a
// structurally-invalid item is a CBOR/parameter error.
const MC_PKCP_ITEM_BAD_CASES: &[RawMcCase] = &[
    RawMcCase {
        // F-1: an item that is NOT a MAP.
        name: "pkcp_item_not_map",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(4),
                    raw::array([
                        raw::map([
                            (raw::text("type"), raw::text("public-key")),
                            (raw::text("alg"), Value::Integer(-7)),
                        ]),
                        Value::Integer(8),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(MALFORMED_ITEM_STATUSES),
    },
    RawMcCase {
        // F-2: an item with "type" missing.
        name: "pkcp_item_type_missing",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(4),
                    raw::array([raw::map([(raw::text("alg"), Value::Integer(-7))])]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(MALFORMED_ITEM_STATUSES),
    },
    RawMcCase {
        // F-3: an item with "type" not TEXT.
        name: "pkcp_item_type_not_text",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(4),
                    raw::array([raw::map([
                        (raw::text("type"), Value::Integer(8)),
                        (raw::text("alg"), Value::Integer(-7)),
                    ])]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(MALFORMED_ITEM_STATUSES),
    },
    RawMcCase {
        // F-4: an item with "alg" missing.
        name: "pkcp_item_alg_missing",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(4),
                    raw::array([raw::map([(raw::text("type"), raw::text("public-key"))])]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(MALFORMED_ITEM_STATUSES),
    },
    RawMcCase {
        // F-5: an item with "alg" not INTEGER.
        name: "pkcp_item_alg_not_int",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(4),
                    raw::array([raw::map([
                        (raw::text("type"), raw::text("public-key")),
                        (raw::text("alg"), raw::text("ES256")),
                    ])]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(MALFORMED_ITEM_STATUSES),
    },
];

#[test]
#[serial]
fn mc_pub_key_cred_params_item_validation() {
    run_in_thread(|| {
        with_authenticator!(mc_pub_key_cred_params_item_validation, |authn| {
            reset_authenticator(authn);
            for case in MC_PKCP_ITEM_BAD_CASES {
                assert_raw_mc_case(authn, case);
            }
        })
    });
}

// --- Authr-MakeCred-Req-4 F-6 / F-7: algorithm selection semantics ---
//
// F-6: pubKeyCredParams contains ONLY an unsupported alg -> UNSUPPORTED_ALGORITHM(0x26).
// F-7: pubKeyCredParams item whose "type" != "public-key" (with otherwise-known
//      alg) is filtered out, leaving an empty list -> UNSUPPORTED_ALGORITHM(0x26).
//
// fido-authenticator filters at deserialization: items with an unknown `type`
// string or an unknown `alg` are dropped from the FilteredPublicKeyCredentialParameters,
// and an empty filtered list yields UnsupportedAlgorithm.
const MC_ALG_SELECTION_CASES: &[RawMcCase] = &[
    RawMcCase {
        // F-6: only-unsupported alg (0x45 == 69, not -7/-8).
        name: "alg_only_unsupported",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(4),
                    raw::array([raw::map([
                        (raw::text("type"), raw::text("public-key")),
                        (raw::text("alg"), Value::Integer(0x45)),
                    ])]),
                );
            })
        },
        expected: ExpectedStatus::Exact(0x26),
    },
    RawMcCase {
        // F-7: type is not "public-key" -> filtered out -> empty list.
        name: "type_not_public_key",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(4),
                    raw::array([raw::map([
                        (raw::text("type"), raw::text("not-a-public-key")),
                        (raw::text("alg"), Value::Integer(-7)),
                    ])]),
                );
            })
        },
        expected: ExpectedStatus::Exact(0x26),
    },
];

#[test]
#[serial]
fn mc_algorithm_selection_errors() {
    run_in_thread(|| {
        with_authenticator!(mc_algorithm_selection_errors, |authn| {
            reset_authenticator(authn);
            for case in MC_ALG_SELECTION_CASES {
                assert_raw_mc_case(authn, case);
            }
        })
    });
}

// --- Authr-MakeCred-Req-4 (positive): selection among multiple supported algs ---
//
// When several supported algorithms are offered, the authenticator must pick
// the FIRST supported one (CTAP 2.1 §6.1.2 step 8 iterates in order). Offer
// [ES256(-7), EdDSA(-8)] and [EdDSA(-8), ES256(-7)] and confirm the response's
// COSE public key alg matches the first offered supported alg.
#[test]
#[serial]
fn mc_pub_key_cred_params_selects_first_supported() {
    run_in_thread(|| {
        with_authenticator!(mc_pub_key_cred_params_selects_first_supported, |authn| {
            reset_authenticator(authn);

            for (algs, expected_alg) in [([-7i32, -8], -7i32), ([-8, -7], -8)] {
                let mut req = make_credential_request();
                req.pub_key_cred_params = pkcp_for(&algs);
                up::approve();
                let resp = authn
                    .call_ctap2(&Request::MakeCredential(req))
                    .expect("MC with multiple algs should succeed");
                match resp {
                    Response::MakeCredential(mc) => {
                        let got = cose_alg_from_auth_data(&mc.auth_data);
                        assert_eq!(
                            got, expected_alg,
                            "expected first supported alg {expected_alg} to be selected, got {got}"
                        );
                    }
                    other => panic!("Expected MakeCredential, got {:?}", other),
                }
            }
        })
    });
}

// --- Authr-MakeCred-Req-5 F-2/F-3/F-5/F-6: malformed excludeList descriptors ---
//
// excludeList items are PublicKeyCredentialDescriptors {type: text, id: bytes}.
// A structurally-malformed descriptor (missing/bad-typed fields) is a parameter
// error. Each excludeList has one valid descriptor plus one malformed one.
fn excl_base_item() -> Value {
    raw::map([
        (raw::text("type"), raw::text("public-key")),
        (raw::text("id"), raw::bytes([0xAB; 32])),
    ])
}

const MC_EXCLUDE_DESCRIPTOR_BAD_CASES: &[RawMcCase] = &[
    RawMcCase {
        // F-2: descriptor with "type" missing.
        name: "excl_type_missing",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(5),
                    raw::array([
                        excl_base_item(),
                        raw::map([(raw::text("id"), raw::bytes([0xCD; 32]))]),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(MALFORMED_ITEM_STATUSES),
    },
    RawMcCase {
        // F-3: descriptor with "type" not TEXT.
        name: "excl_type_not_text",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(5),
                    raw::array([
                        excl_base_item(),
                        raw::map([
                            (raw::text("type"), Value::Integer(8)),
                            (raw::text("id"), raw::bytes([0xCD; 32])),
                        ]),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(MALFORMED_ITEM_STATUSES),
    },
    RawMcCase {
        // F-5: descriptor with "id" missing.
        name: "excl_id_missing",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(5),
                    raw::array([
                        excl_base_item(),
                        raw::map([(raw::text("type"), raw::text("public-key"))]),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(MALFORMED_ITEM_STATUSES),
    },
    RawMcCase {
        // F-6: descriptor with "id" not BYTE STRING.
        name: "excl_id_not_bytes",
        request: || {
            raw_mc_value(|m| {
                m.insert(
                    raw::int_key(5),
                    raw::array([
                        excl_base_item(),
                        raw::map([
                            (raw::text("type"), raw::text("public-key")),
                            (raw::text("id"), raw::text("not-bytes")),
                        ]),
                    ]),
                );
            })
        },
        expected: ExpectedStatus::OneOf(MALFORMED_ITEM_STATUSES),
    },
];

#[test]
#[serial]
fn mc_exclude_list_descriptor_validation() {
    run_in_thread(|| {
        with_authenticator!(mc_exclude_list_descriptor_validation, |authn| {
            reset_authenticator(authn);
            for case in MC_EXCLUDE_DESCRIPTOR_BAD_CASES {
                assert_raw_mc_case(authn, case);
            }
        })
    });
}

// --- Authr-MakeCred-Req-5 P-1: excludeList with an unknown-type descriptor is ignored ---
//
// mc_group_basic covers a single unknown-type descriptor that does not match.
// Here we mirror the conformance P-1 shape exactly: a TWO-element excludeList
// containing one well-formed (non-matching) public-key descriptor AND one
// descriptor with an unknown `type` string. The unknown-type entry must be
// silently ignored and registration must still SUCCEED.
#[test]
#[serial]
fn mc_exclude_list_unknown_type_ignored() {
    run_in_thread(|| {
        with_authenticator!(mc_exclude_list_unknown_type_ignored, |authn| {
            reset_authenticator(authn);

            let mut req = make_credential_request();
            let mut list = ctap_types::Vec::new();
            // Well-formed, non-matching public-key descriptor.
            list.push(descriptor_ref(&[0x11; 32])).unwrap();
            // Unknown credential type -> must be ignored.
            list.push(descriptor_ref_typed(&[0x22; 32], "mangoPapayaCoconut"))
                .unwrap();
            req.exclude_list = Some(list);
            up::approve();
            authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("excludeList with unknown-type descriptor should still succeed");
        })
    });
}

// --- Authr-MakeCred-Req-6 P-1: unknown option is ignored ---
//
// An unrecognised option key ("makeTea") must be ignored, and registration
// succeeds. The typed Request builder can't carry an arbitrary option key, so
// build the options map via raw CBOR.
#[test]
#[serial]
fn mc_unknown_option_ignored() {
    run_in_thread(|| {
        with_authenticator!(mc_unknown_option_ignored, |authn| {
            reset_authenticator(authn);

            let payload = raw_mc_payload(raw_mc_value(|m| {
                m.insert(
                    raw::int_key(7),
                    raw::map([(raw::text("makeTea"), Value::Bool(true))]),
                );
            }));
            up::approve();
            let (status, _body) = authn
                .call_ctap2_raw(mc_command(), &payload)
                .expect("raw MakeCredential transport failed");
            assert_eq!(
                status, 0x00,
                "unknown option should be ignored, expected success"
            );
        })
    });
}

// --- Authr-MakeCred-Req-6 P-3: options.up=true sets the UP flag ---
#[test]
#[serial]
fn mc_option_up_true_sets_flag() {
    run_in_thread(|| {
        with_authenticator!(mc_option_up_true_sets_flag, |authn| {
            reset_authenticator(authn);

            let mut req = make_credential_request();
            req.options = Some(decode_from_value(options_value(None, Some(true), None)));
            up::approve();
            let resp = authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("MC with options.up=true should succeed");
            match resp {
                Response::MakeCredential(mc) => {
                    assert!(
                        mc.auth_data[32] & 0x01 != 0,
                        "UP flag must be set when options.up=true"
                    );
                }
                other => panic!("Expected MakeCredential, got {:?}", other),
            }
        })
    });
}

// --- Authr-MakeCred-Req-6 F-1: options.up=false -> CTAP2_ERR_INVALID_OPTION(0x2C) ---
//
// makeCredential does not permit up=false (UP is mandatory for registration).
#[test]
#[serial]
fn mc_option_up_false_is_invalid_option() {
    run_in_thread(|| {
        with_authenticator!(mc_option_up_false_is_invalid_option, |authn| {
            reset_authenticator(authn);

            let mut req = make_credential_request();
            req.options = Some(decode_from_value(options_value(None, Some(false), None)));
            let result = authn.call_ctap2(&Request::MakeCredential(req));
            assert_eq!(
                result,
                Err(ctap2::Error::InvalidOption),
                "options.up=false must fail with InvalidOption(0x2C)"
            );
        })
    });
}

// --- Authr-MakeCred-Req-6: rk=true creates a discoverable credential ---
//
// Smoke-cover the rk option (true) which the device advertises. A resident-key
// registration must succeed; combined with the existing resident_key.rs suite
// this confirms options.rk is accepted at the MakeCredential boundary.
#[test]
#[serial]
fn mc_option_rk_true_succeeds() {
    run_in_thread(|| {
        with_authenticator!(mc_option_rk_true_succeeds, |authn| {
            reset_authenticator(authn);

            let mut req = make_credential_request();
            req.options = Some(decode_from_value(options_value(Some(true), None, None)));
            up::approve();
            authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("MC with options.rk=true should succeed");
        })
    });
}

// --- Authr-MakeCred-Req-6 / GetInfo: uv=true without a configured PIN ---
//
// The device advertises makeCredUvNotRqd and alwaysUv=false, and does NOT have
// a built-in UV. With no PIN set, requesting options.uv=true cannot be honoured.
// Per CTAP 2.1, an authenticator with no built-in UV and no clientPin set must
// reject uv=true. Accept the spec-permitted error families.
#[test]
#[serial]
fn mc_option_uv_true_without_pin_errors() {
    run_in_thread(|| {
        with_authenticator!(mc_option_uv_true_without_pin_errors, |authn| {
            reset_authenticator(authn);

            let payload = raw_mc_payload(raw_mc_value(|m| {
                m.insert(
                    raw::int_key(7),
                    raw::map([(raw::text("uv"), Value::Bool(true))]),
                );
            }));
            let (status, _body) = authn
                .call_ctap2_raw(mc_command(), &payload)
                .expect("raw MakeCredential transport failed");
            // 0x2B UnsupportedOption, 0x2C InvalidOption, or 0x36 PinRequired
            // are all spec-acceptable when uv is requested without any UV
            // configured. It must NOT silently succeed.
            assert!(
                [0x2B, 0x2C, 0x36].contains(&status),
                "uv=true without PIN/built-in-UV expected UnsupportedOption/InvalidOption/PinRequired, got 0x{status:02x}"
            );
        })
    });
}

// --- Authr-MakeCred-Resp-1 P-01/P-02/P-03/F-01: response & attestation structure ---
//
// Deepen the structural checks on a successful registration response:
//   P-01 fmt == "packed" (already in mc_group_basic; re-asserted here in context)
//   P-02 authData length, AAGUID present, UP+AT flags set, ED flag clear,
//        COSE public key parses with a known kty and the expected alg
//   P-03 attStmt is present with an "alg" matching the credential public key
//        and a non-empty "sig" byte string
//   F-01 unsignedExtensionOutputs (resp key 0x06) absent for a no-extension cred
//
// We re-encode the typed Response to a CBOR map to inspect raw keys for the
// attStmt/unsignedExtensionOutputs checks, and parse auth_data bytes directly
// for the authData structural checks.
#[test]
#[serial]
fn mc_response_structure_packed() {
    run_in_thread(|| {
        with_authenticator!(mc_response_structure_packed, |authn| {
            reset_authenticator(authn);

            up::approve();
            let resp = authn
                .call_ctap2(&Request::MakeCredential(make_credential_request()))
                .expect("MC should succeed");
            let mc = match resp {
                Response::MakeCredential(mc) => mc,
                other => panic!("Expected MakeCredential, got {:?}", other),
            };

            // P-01: fmt == packed.
            assert_eq!(
                mc.fmt,
                ctap2::AttestationStatementFormat::Packed,
                "fmt must be packed"
            );

            // P-02: authData structural checks.
            let ad = &mc.auth_data;
            assert!(
                ad.len() >= 32 + 1 + 4 + 16 + 2 + 16 + 77,
                "authData must be at least 146 bytes, got {}",
                ad.len()
            );
            let flags = ad[32];
            assert!(flags & 0x01 != 0, "UP flag must be set");
            assert!(flags & 0x40 != 0, "AT flag must be set");
            assert!(
                flags & 0x80 == 0,
                "ED flag must be clear for a no-extension credential"
            );
            // AAGUID is the 16 bytes after rpIdHash(32)+flags(1)+signCount(4).
            // (No assertion on its value here; cred_protect/get_info cover AAGUID
            // identity. We only assert the attested-credential-data region is
            // present and the COSE key parses.)
            let cose_alg = cose_alg_from_auth_data(ad);
            assert!(
                cose_alg == -7 || cose_alg == -8,
                "COSE public key alg must be a supported alg (-7 or -8), got {cose_alg}"
            );

            // P-03 + F-01: inspect raw response keys.
            let resp_value: Value = {
                let encoded = serde_cbor::to_vec(&mc).expect("serialize MC response");
                serde_cbor::from_slice(&encoded).expect("decode MC response to Value")
            };
            let map = match &resp_value {
                Value::Map(m) => m,
                other => panic!("MC response is not a map: {:?}", other),
            };
            let get = |k: i128| map.get(&Value::Integer(k));

            // F-01: unsignedExtensionOutputs (0x06) must be absent.
            assert!(
                get(0x06).is_none(),
                "unsignedExtensionOutputs must be absent for a no-extension credential"
            );

            // P-03: attStmt (0x03) present, with alg matching the credential
            // public key and a non-empty sig byte string.
            let att_stmt = get(0x03).expect("attStmt (0x03) must be present");
            let att_map = match att_stmt {
                Value::Map(m) => m,
                other => panic!("attStmt is not a map: {:?}", other),
            };
            match att_map.get(&Value::Text("alg".to_string())) {
                Some(Value::Integer(alg)) => assert_eq!(
                    *alg as i32, cose_alg,
                    "attStmt.alg must match credential public key alg"
                ),
                other => panic!("attStmt.alg must be an integer, got {:?}", other),
            }
            match att_map.get(&Value::Text("sig".to_string())) {
                Some(Value::Bytes(sig)) => {
                    assert!(
                        !sig.is_empty(),
                        "attStmt.sig must be a non-empty byte string"
                    )
                }
                other => panic!("attStmt.sig must be a byte string, got {:?}", other),
            }
        })
    });
}

/// Parse the COSE public key `alg` (label 3) out of a MakeCredential `authData`.
///
/// Layout: rpIdHash(32) + flags(1) + signCount(4) + AAGUID(16) +
/// credIdLen(2) + credId(credIdLen) + COSE_Key(CBOR map). The COSE key is the
/// remaining bytes; decode it as a CBOR map and read integer label 3.
fn cose_alg_from_auth_data(auth_data: &[u8]) -> i32 {
    let cred_id_len =
        u16::from_be_bytes([auth_data[32 + 1 + 4 + 16], auth_data[32 + 1 + 4 + 16 + 1]]) as usize;
    let cose_start = 32 + 1 + 4 + 16 + 2 + cred_id_len;
    let cose: Value =
        serde_cbor::from_slice(&auth_data[cose_start..]).expect("decode COSE public key");
    let map = match cose {
        Value::Map(m) => m,
        other => panic!("COSE public key is not a map: {:?}", other),
    };
    match map.get(&Value::Integer(3)) {
        Some(Value::Integer(alg)) => *alg as i32,
        other => panic!("COSE public key alg (label 3) missing/invalid: {:?}", other),
    }
}
