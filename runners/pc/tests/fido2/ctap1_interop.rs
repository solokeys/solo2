//! CTAP1 ↔ CTAP2 cross-credential interop.
//!
//! Mirrors `fido2-tests/tests/standard/fido2/test_ctap1_interop.py`. Two
//! directions:
//!
//!   1. **CTAP1 → CTAP2**: register a credential via U2F `REGISTER`, then
//!      use the resulting `keyHandle` in a CTAP2 `GetAssertion` allow_list.
//!   2. **CTAP2 → CTAP1**: register a credential via CTAP2 `MakeCredential`,
//!      then authenticate via U2F `AUTHENTICATE` (only works when the CTAP2
//!      credId fits in U2F's 255-byte key-handle field).
//!
//! Both paths exercise fido-authenticator's CTAP1↔CTAP2 credential bridge.

use super::*;

const INS_REGISTER: u8 = 0x01;
const INS_AUTHENTICATE: u8 = 0x02;
const P1_ENFORCE_USER_PRESENCE_AND_SIGN: u8 = 0x03;
const SW_NO_ERROR: u16 = 0x9000;

const CHALLENGE: [u8; 32] = [0x33; 32];
// fido-authenticator hashes the rp_id to derive the U2F appid binding. To
// keep the CTAP1 and CTAP2 sides aligned, set the U2F appid to sha256 of
// the CTAP2 rp_id we use in the GA below.
const RP_ID: &str = "example.com";

fn appid_from_rp_id(rp_id: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(rp_id.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn build_apdu(cla: u8, ins: u8, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
    let mut apdu = vec![cla, ins, p1, p2];
    apdu.push(0);
    apdu.push((data.len() >> 8) as u8);
    apdu.push((data.len() & 0xff) as u8);
    apdu.extend_from_slice(data);
    apdu.extend_from_slice(&[0, 0]);
    apdu
}

fn u2f_register(authn: &mut dyn TestAuthenticator, appid: &[u8; 32]) -> (Vec<u8>, Vec<u8>) {
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(&CHALLENGE);
    data.extend_from_slice(appid);
    up::approve();
    let (sw, payload) = authn
        .call_ctap1_apdu(&build_apdu(0x00, INS_REGISTER, 0x00, 0x00, &data))
        .expect("U2F register transport");
    assert_eq!(sw, SW_NO_ERROR);

    // Layout: 0x05 | pubKey(65) | khLen(1) | keyHandle(khLen) | cert | sig
    assert_eq!(payload[0], 0x05);
    let public_key = payload[1..1 + 65].to_vec();
    let kh_len = payload[1 + 65] as usize;
    let kh_start = 1 + 65 + 1;
    let key_handle = payload[kh_start..kh_start + kh_len].to_vec();
    (public_key, key_handle)
}

/// CTAP1 → CTAP2: register via U2F, then assert via CTAP2 GA using the
/// keyHandle as a public-key descriptor.
#[test]
#[serial]
fn ctap1_register_then_ctap2_get_assertion() {
    run_in_thread(|| {
        with_authenticator!(ctap1_to_ctap2, |authn| {
            reset_authenticator(authn);
            let appid = appid_from_rp_id(RP_ID);
            let (_pub_key, key_handle) = u2f_register(authn, &appid);

            up::approve();
            let resp = authn
                .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                    RP_ID,
                    Some(single_allow_list(&key_handle)),
                )))
                .expect("CTAP2 GA on U2F-registered keyHandle should succeed");
            match resp {
                Response::GetAssertion(ga) => {
                    assert_eq!(
                        ga.credential.id.to_vec(),
                        key_handle,
                        "returned credential id must match the U2F key handle",
                    );
                }
                other => panic!("Expected GetAssertion, got {:?}", other),
            }
        })
    });
}

/// CTAP2 → CTAP1: register via CTAP2 MC, then authenticate via U2F using
/// the credId as the key handle. Only valid when the credId fits the U2F
/// 255-byte limit; fido-authenticator's credIds easily fit.
#[test]
#[serial]
fn ctap2_register_then_ctap1_authenticate() {
    run_in_thread(|| {
        with_authenticator!(ctap2_to_ctap1, |authn| {
            reset_authenticator(authn);
            up::approve();
            let credential_id = make_credential(authn);
            assert!(
                credential_id.len() <= 255,
                "credId longer than U2F key-handle field — skip on real hardware",
            );

            let appid = appid_from_rp_id(RP_ID);
            let mut data = Vec::with_capacity(65 + credential_id.len());
            data.extend_from_slice(&CHALLENGE);
            data.extend_from_slice(&appid);
            data.push(credential_id.len() as u8);
            data.extend_from_slice(&credential_id);

            up::approve();
            let (sw, payload) = authn
                .call_ctap1_apdu(&build_apdu(
                    0x00,
                    INS_AUTHENTICATE,
                    P1_ENFORCE_USER_PRESENCE_AND_SIGN,
                    0x00,
                    &data,
                ))
                .expect("U2F AUTHENTICATE on CTAP2-registered cred should succeed");
            assert_eq!(sw, SW_NO_ERROR);
            assert!(
                payload.len() >= 5 + 64,
                "U2F auth payload must include UP byte, counter, and signature",
            );
            assert_eq!(
                payload[0], 0x01,
                "user-presence byte must be set after up::approve()",
            );
        })
    });
}
