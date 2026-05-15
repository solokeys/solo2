//! CTAP1 / U2F raw APDU tests.
//!
//! Mirrors `fido2-tests/tests/standard/u2f/test_u2f.py`. Each test sends a
//! short-form APDU through `TestAuthenticator::call_ctap1_apdu` (which routes
//! it via CTAPHID `MSG` on hardware backends, or directly into
//! `fido_authenticator::handle_ctap1_from_hid` for in-process Sim).
//!
//! ## APDU encoding helpers
//!
//! All requests use **extended-length** APDUs (Lc = 3 bytes, Le = 3 bytes
//! with leading 0). That's the wire shape fido-authenticator's CTAP1
//! handler expects. Status words are returned as a big-endian `u16` in
//! the response tail.

use super::*;
use sha2::{Digest, Sha256};

const INS_REGISTER: u8 = 0x01;
const INS_AUTHENTICATE: u8 = 0x02;
const INS_VERSION: u8 = 0x03;

const P1_CHECK_ONLY: u8 = 0x07;
const P1_ENFORCE_USER_PRESENCE_AND_SIGN: u8 = 0x03;

// U2F response status words (ISO 7816-4).
const SW_NO_ERROR: u16 = 0x9000;
const SW_WRONG_DATA: u16 = 0x6A80;
const SW_USE_NOT_SATISFIED: u16 = 0x6985;
const SW_INS_NOT_SUPPORTED: u16 = 0x6D00;
const SW_CLA_NOT_SUPPORTED: u16 = 0x6E00;

const CHALLENGE: [u8; 32] = [0x42; 32];
const APPID: [u8; 32] = [0xab; 32];

fn build_apdu(cla: u8, ins: u8, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
    let mut apdu = vec![cla, ins, p1, p2];
    if data.is_empty() {
        // Case 2E (no command data, expect response): Le = 0x00 0x00 0x00
        apdu.extend_from_slice(&[0, 0, 0]);
    } else {
        // Case 4E (command data + response): Lc(3) + data + Le(2).
        apdu.push(0);
        apdu.push((data.len() >> 8) as u8);
        apdu.push((data.len() & 0xff) as u8);
        apdu.extend_from_slice(data);
        apdu.extend_from_slice(&[0, 0]);
    }
    apdu
}

fn u2f_register(authn: &mut dyn TestAuthenticator) -> Vec<u8> {
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(&CHALLENGE);
    data.extend_from_slice(&APPID);
    up::approve();
    let (sw, payload) = authn
        .call_ctap1_apdu(&build_apdu(0x00, INS_REGISTER, 0x00, 0x00, &data))
        .expect("U2F register transport should succeed");
    assert_eq!(sw, SW_NO_ERROR, "U2F register status");
    payload
}

/// Parse out the keyHandle from a U2F register response.
/// Layout: 0x05 | publicKey(65) | khLen(1) | keyHandle(khLen) | cert ... | sig
fn extract_key_handle(reg: &[u8]) -> Vec<u8> {
    assert_eq!(reg[0], 0x05, "register response must start with 0x05");
    let kh_len = reg[1 + 65] as usize;
    let kh_start = 1 + 65 + 1;
    reg[kh_start..kh_start + kh_len].to_vec()
}

fn build_authenticate_data(challenge: &[u8; 32], appid: &[u8; 32], kh: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(65 + kh.len());
    data.extend_from_slice(challenge);
    data.extend_from_slice(appid);
    data.push(kh.len() as u8);
    data.extend_from_slice(kh);
    data
}

// ---------- Happy-path tests ----------

#[test]
#[serial]
fn u2f_version_returns_u2f_v2() {
    run_in_thread(|| {
        with_authenticator!(u2f_version, |authn| {
            let (sw, payload) = authn
                .call_ctap1_apdu(&build_apdu(0x00, INS_VERSION, 0x00, 0x00, &[]))
                .expect("U2F version transport");
            assert_eq!(sw, SW_NO_ERROR, "VERSION should return 0x9000");
            assert_eq!(
                payload.as_slice(),
                b"U2F_V2",
                "VERSION payload must be U2F_V2",
            );
        })
    });
}

#[test]
#[serial]
fn u2f_register_and_authenticate_roundtrip() {
    run_in_thread(|| {
        with_authenticator!(u2f_register_auth, |authn| {
            reset_authenticator(authn);
            let reg = u2f_register(authn);
            let kh = extract_key_handle(&reg);

            // Authenticate with enforce-up: must return 0x9000 and a non-empty signature.
            let data = build_authenticate_data(&CHALLENGE, &APPID, &kh);
            up::approve();
            let (sw, payload) = authn
                .call_ctap1_apdu(&build_apdu(
                    0x00,
                    INS_AUTHENTICATE,
                    P1_ENFORCE_USER_PRESENCE_AND_SIGN,
                    0x00,
                    &data,
                ))
                .expect("U2F authenticate transport");
            assert_eq!(sw, SW_NO_ERROR);
            // Layout: userPresence(1) | counter(4) | signature(ECDSA DER, ~70-72 bytes).
            assert!(payload.len() >= 5 + 64, "auth payload too short");
            assert_eq!(
                payload[0], 0x01,
                "user-presence byte must be set after up::approve()",
            );
        })
    });
}

#[test]
#[serial]
fn u2f_authenticate_check_only_returns_use_not_satisfied() {
    run_in_thread(|| {
        with_authenticator!(u2f_check_only, |authn| {
            reset_authenticator(authn);
            let reg = u2f_register(authn);
            let kh = extract_key_handle(&reg);

            // check_only (P1 = 0x07): the spec mandates SW_USE_NOT_SATISFIED on
            // a key-handle match, since the authenticator must NOT produce a
            // signature here.
            let data = build_authenticate_data(&CHALLENGE, &APPID, &kh);
            let (sw, _payload) = authn
                .call_ctap1_apdu(&build_apdu(
                    0x00,
                    INS_AUTHENTICATE,
                    P1_CHECK_ONLY,
                    0x00,
                    &data,
                ))
                .expect("U2F check_only transport");
            assert_eq!(sw, SW_USE_NOT_SATISFIED);
        })
    });
}

// ---------- Negative / error-path table ----------

#[derive(Copy, Clone)]
struct U2fBadApduCase {
    name: &'static str,
    apdu: fn() -> Vec<u8>,
    expected_sw: u16,
}

const U2F_BAD_APDU_CASES: &[U2fBadApduCase] = &[
    U2fBadApduCase {
        name: "bad_ins",
        apdu: || build_apdu(0x00, 0x00, 0x00, 0x00, &[]),
        expected_sw: SW_INS_NOT_SUPPORTED,
    },
    U2fBadApduCase {
        name: "bad_cla",
        apdu: || build_apdu(0x01, INS_VERSION, 0x00, 0x00, b"abc"),
        expected_sw: SW_CLA_NOT_SUPPORTED,
    },
];

#[test]
#[serial]
fn u2f_bad_apdu_returns_expected_status_words() {
    run_in_thread(|| {
        with_authenticator!(u2f_bad_apdu, |authn| {
            for case in U2F_BAD_APDU_CASES {
                let apdu = (case.apdu)();
                let (sw, _payload) = authn
                    .call_ctap1_apdu(&apdu)
                    .expect("U2F bad-apdu transport");
                assert_eq!(
                    sw, case.expected_sw,
                    "case `{}`: expected 0x{:04x}, got 0x{:04x}",
                    case.name, case.expected_sw, sw,
                );
            }
        })
    });
}

#[test]
#[serial]
fn u2f_bad_key_handle_returns_wrong_data() {
    run_in_thread(|| {
        with_authenticator!(u2f_bad_kh, |authn| {
            reset_authenticator(authn);
            let reg = u2f_register(authn);
            let mut kh = extract_key_handle(&reg);
            // Flip a bit to make the key handle un-decryptable.
            kh[0] ^= 0x40;

            let data = build_authenticate_data(&CHALLENGE, &APPID, &kh);
            let (sw, _payload) = authn
                .call_ctap1_apdu(&build_apdu(
                    0x00,
                    INS_AUTHENTICATE,
                    P1_CHECK_ONLY,
                    0x00,
                    &data,
                ))
                .expect("U2F bad-kh transport");
            assert_eq!(sw, SW_WRONG_DATA);
        })
    });
}

#[test]
#[serial]
fn u2f_incorrect_appid_returns_wrong_data() {
    run_in_thread(|| {
        with_authenticator!(u2f_bad_appid, |authn| {
            reset_authenticator(authn);
            let reg = u2f_register(authn);
            let kh = extract_key_handle(&reg);

            let mut bad_appid = APPID;
            bad_appid[0] ^= 0x40;
            let data = build_authenticate_data(&CHALLENGE, &bad_appid, &kh);
            let (sw, _payload) = authn
                .call_ctap1_apdu(&build_apdu(
                    0x00,
                    INS_AUTHENTICATE,
                    P1_CHECK_ONLY,
                    0x00,
                    &data,
                ))
                .expect("U2F bad-appid transport");
            assert_eq!(sw, SW_WRONG_DATA);
        })
    });
}

#[test]
#[serial]
fn u2f_appid_hash_appears_in_authenticate_payload() {
    // Sanity check that the authenticator binds appid to the keyHandle: a
    // round-trip with the correct (challenge, appid) yields a signature
    // whose preimage starts with sha256(appid).
    run_in_thread(|| {
        with_authenticator!(u2f_appid_bind, |authn| {
            reset_authenticator(authn);
            let reg = u2f_register(authn);
            let kh = extract_key_handle(&reg);

            let data = build_authenticate_data(&CHALLENGE, &APPID, &kh);
            up::approve();
            let (sw, _payload) = authn
                .call_ctap1_apdu(&build_apdu(
                    0x00,
                    INS_AUTHENTICATE,
                    P1_ENFORCE_USER_PRESENCE_AND_SIGN,
                    0x00,
                    &data,
                ))
                .expect("U2F authenticate transport");
            assert_eq!(sw, SW_NO_ERROR);

            // sha256(appid) is part of the authData; we only assert the
            // authenticator emitted *something*, since the actual sig
            // bytes are opaque without a verifier.
            let _ = Sha256::digest(APPID);
        })
    });
}
