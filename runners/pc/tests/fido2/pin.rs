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
                // CTAP 2.1 §6.5.5.4 step 3: setPin against an already-provisioned
                // authenticator returns PinAuthInvalid. (CTAP 2.0 returned NotAllowed.)
                assert_eq!(
                    PinSession::try_set_pin(authn, "1234"),
                    Err(ctap2::Error::PinAuthInvalid)
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

// CTAP 2.1 §6.5.5.7 step 2 / upstream PR #56: a zero-length pinAuth with a
// PIN set returns PinInvalid (CTAP 2.0 returned PinAuthInvalid).
const EMPTY_PIN_AUTH_CASES: &[EmptyPinAuthCase] = &[
    EmptyPinAuthCase {
        name: "make_credential",
        command: 0x01,
        request: raw_empty_pin_mc_request,
        expected: ctap2::Error::PinInvalid,
    },
    EmptyPinAuthCase {
        name: "get_assertion",
        command: 0x02,
        request: raw_empty_pin_ga_request,
        expected: ctap2::Error::PinInvalid,
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

/// CTAP 2.1 §6.5.5.1: getKeyAgreement returns a COSE_Key with
/// kty=EC2 (2), crv=P256 (1), alg=ECDH_ES_HKDF_256 (-25), and a 32-byte x/y.
#[test]
#[serial]
fn pin_get_key_agreement_fields() {
    run_in_thread(|| {
        with_authenticator!(pin_get_key_agreement_fields, |authn| {
            let key_agreement = support::pin::get_authenticator_key_agreement(authn);
            assert_eq!(key_agreement.x.len(), 32, "x coordinate must be 32 bytes");
            assert_eq!(key_agreement.y.len(), 32, "y coordinate must be 32 bytes");
        })
    });
}

/// CTAP 2.1 §6.1.2 step 6: with `makeCredUvNotRqd` advertised (fido-authenticator
/// does), a non-rk MC without `pinUvAuthParam` succeeds with the UV bit clear,
/// even when a PIN is set. A non-rk GA without `pinUvAuthParam` likewise
/// succeeds with UV clear.
#[test]
#[serial]
fn pin_unauth_request_omits_uv_flag() {
    run_isolated_in_sim("pin::pin_unauth_request_omits_uv_flag", || {
        run_in_thread(|| {
            with_authenticator!(pin_unauth_request, |authn| {
                reset_authenticator(authn);
                PinSession::set_pin(authn, PIN1);

                up::approve();
                let credential_id = make_credential(authn);

                up::approve();
                let assertion = get_assertion_with_pin(authn, &credential_id, None);
                assert_eq!(
                    assertion.auth_data[32] & (1 << 2),
                    0,
                    "GA without pinAuth should leave UV bit clear",
                );

                up::approve();
                let mc = match authn
                    .call_ctap2(&Request::MakeCredential(make_credential_request_for(
                        "no-pin.example",
                        &[0xaa; 16],
                        "no-pin-user",
                        false,
                    )))
                    .expect("MC without pinAuth should succeed under makeCredUvNotRqd")
                {
                    Response::MakeCredential(mc) => mc,
                    other => panic!("Expected MakeCredential, got {:?}", other),
                };
                assert_eq!(
                    mc.auth_data[32] & (1 << 2),
                    0,
                    "MC without pinAuth should leave UV bit clear",
                );
            })
        });
    });
}

/// MC with a PIN protocol/auth sets the UV bit in auth_data.
#[test]
#[serial]
fn pin_make_credential_sets_uv_flag() {
    run_isolated_in_sim("pin::pin_make_credential_sets_uv_flag", || {
        run_in_thread(|| {
            with_authenticator!(pin_make_credential_sets_uv_flag, |authn| {
                reset_authenticator(authn);
                PinSession::set_pin(authn, PIN1);
                let pin = PinSession::get_pin_token(authn, PIN1);

                let mut request = make_credential_request();
                request.pin_protocol = Some(pin.protocol().into());
                let pin_auth = pin.pin_auth_for_client_data_hash(request.client_data_hash.as_ref());
                request.pin_auth = Some(leak_bytes(pin_auth.to_vec()));

                up::approve();
                let mc = match authn
                    .call_ctap2(&Request::MakeCredential(request))
                    .expect("MC with pinAuth should succeed")
                {
                    Response::MakeCredential(mc) => mc,
                    other => panic!("Expected MakeCredential, got {:?}", other),
                };
                assert!(
                    mc.auth_data[32] & (1 << 2) != 0,
                    "MC with pinAuth must set UV bit",
                );
            })
        });
    });
}

/// GA with no matching credential returns NoCredentials, even with a PIN set.
#[test]
#[serial]
fn pin_get_assertion_no_credential_yields_no_credentials() {
    run_isolated_in_sim(
        "pin::pin_get_assertion_no_credential_yields_no_credentials",
        || {
            run_in_thread(|| {
                with_authenticator!(pin_ga_no_cred, |authn| {
                    reset_authenticator(authn);
                    PinSession::set_pin(authn, PIN1);

                    up::approve();
                    let result = authn.call_ctap2(&Request::GetAssertion(
                        get_assertion_request_for("example.com", None),
                    ));
                    assert_eq!(result, Err(ctap2::Error::NoCredentials));
                })
            });
        },
    );
}

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

// =============================================================================
// APPENDED: cases ported from the CTAP2.3 conformance ClientPin1/ClientPin2
// modules that were not yet covered above. Existing fns above are unchanged.
//
// Conformance source:
//   tests/CTAP2/Protocol/ClientPin/ClientPin1/Authr-ClientPin1-*.js
//   tests/CTAP2/Protocol/ClientPin/ClientPin2/Authr-ClientPin2-*.js
//   js/ClientPin2Utils.js
// =============================================================================

// --- ClientPin1 (PIN protocol 1) — gaps over the existing fns -----------------

/// ClientPin1-GetRetries P-2 (tail): after two failed `getPinToken` attempts
/// the retries counter has decremented by two; a subsequent SUCCESSFUL PIN
/// authentication (MakeCredential with a valid pinAuth) MUST reset the retries
/// counter back to the original value (8).
#[test]
#[serial]
fn pin1_retries_reset_after_successful_auth() {
    run_isolated_in_sim("pin::pin1_retries_reset_after_successful_auth", || {
        run_in_thread(|| {
            with_authenticator!(pin1_retries_reset_after_successful_auth, |authn| {
                reset_authenticator(authn);
                PinSession::set_pin(authn, PIN1);

                let original = pin::get_retries(authn);
                assert_eq!(original, 8, "fresh PIN should start at 8 retries");

                // Two bad getPinToken attempts -> retries decrease by two.
                for expected_after in [7u8, 6u8] {
                    assert_eq!(
                        PinSession::try_get_pin_token(authn, "wrong-pin").map(|_| ()),
                        Err(ctap2::Error::PinInvalid),
                    );
                    assert_eq!(pin::get_retries(authn), expected_after);
                }

                // A successful PIN-authenticated MakeCredential resets retries.
                let pin = PinSession::get_pin_token(authn, PIN1);
                let _ = make_credential_with_pin(authn, &pin);
                assert_eq!(
                    pin::get_retries(authn),
                    original,
                    "retries must reset to the original counter after a successful PIN auth",
                );
            })
        });
    });
}

/// ClientPin1-PinPolicy F-1: setting a new PIN shorter than the 4-codepoint
/// minimum MUST fail with CTAP2_ERR_PIN_POLICY_VIOLATION. (F-2/F-3 — PIN of 64
/// bytes — are already covered by `pin_setup_and_get_info`'s `set_pin_too_long`
/// case; this complements them at the lower bound.)
#[test]
#[serial]
fn pin1_set_pin_too_short_rejected() {
    run_isolated_in_sim("pin::pin1_set_pin_too_short_rejected", || {
        run_in_thread(|| {
            with_authenticator!(pin1_set_pin_too_short_rejected, |authn| {
                reset_authenticator(authn);
                // 3 codepoints < minimum of 4.
                assert_eq!(
                    PinSession::try_set_pin(authn, "123"),
                    Err(ctap2::Error::PinPolicyViolation),
                );
            })
        });
    });
}

// --- ClientPin2 (PIN protocol 2) ---------------------------------------------
//
// `support::pin::PinSession` is hard-wired to PIN protocol 1, so the protocol-2
// variants below carry a self-contained protocol-2 client. Protocol 2 differs
// from protocol 1 in its key schedule and symmetric layer (CTAP 2.1 §6.5.6/§6.5.7):
//   * sharedSecret = HKDF-SHA256(salt=0x00*32, ikm=ECDH-x) split into a 32-byte
//     HMAC key (info="CTAP2 HMAC key") and a 32-byte AES key (info="CTAP2 AES key");
//   * encryption is AES-256-CBC with a fresh random 16-byte IV PREPENDED to the
//     ciphertext;
//   * pinUvAuthParam is the FULL 32-byte HMAC-SHA-256 (protocol 1 truncates to 16).
//
// All protocol-2 ClientPin requests are sent through `call_ctap2_raw(0x06, ..)`
// and the response map is parsed as `serde_cbor::Value`, mirroring how the
// conformance harness asserts on `ctap2Response.statusCode` / `cborResponse`.

mod p2 {
    use super::*;
    use hmac::{Hmac, Mac};
    use p256::ecdh::diffie_hellman;
    use p256::elliptic_curve::sec1::ToEncodedPoint;
    use p256::{PublicKey, SecretKey};
    use rand_core::{OsRng, RngCore};
    use serde_cbor::Value;
    use sha2::{Digest, Sha256};

    use aes::Aes256;
    use cbc::cipher::{block_padding::NoPadding, BlockDecryptMut, BlockEncryptMut, KeyIvInit};

    type HmacSha256 = Hmac<Sha256>;
    type Aes256CbcEnc = cbc::Encryptor<Aes256>;
    type Aes256CbcDec = cbc::Decryptor<Aes256>;

    const SUB_GET_RETRIES: i128 = 0x01;
    const SUB_GET_KEY_AGREEMENT: i128 = 0x02;
    const SUB_SET_PIN: i128 = 0x03;
    const SUB_CHANGE_PIN: i128 = 0x04;
    const SUB_GET_PIN_TOKEN: i128 = 0x05;
    const SUB_GET_TOKEN_USING_PIN_PERMS: i128 = 0x09;

    pub struct SharedSecret {
        pub hmac_key: [u8; 32],
        pub aes_key: [u8; 32],
        pub platform_public: PublicKey,
    }

    /// HKDF-SHA256, single 32-byte output block (sufficient for both keys).
    fn hkdf_sha256_32(salt: &[u8], ikm: &[u8], info: &[u8]) -> [u8; 32] {
        // Extract.
        let mut extract = HmacSha256::new_from_slice(salt).unwrap();
        extract.update(ikm);
        let prk = extract.finalize().into_bytes();
        // Expand (T(1) only: info || 0x01).
        let mut expand = HmacSha256::new_from_slice(&prk).unwrap();
        expand.update(info);
        expand.update(&[0x01]);
        let okm = expand.finalize().into_bytes();
        let mut out = [0u8; 32];
        out.copy_from_slice(&okm[..32]);
        out
    }

    /// Send a raw ClientPin (0x06) protocol-2 request and return the status byte
    /// plus the decoded response map.
    fn call(
        authn: &mut dyn TestAuthenticator,
        entries: Vec<(Value, Value)>,
    ) -> (u8, std::collections::BTreeMap<Value, Value>) {
        let payload = support::raw::encode(&Value::Map(entries.into_iter().collect()));
        let (status, body) = authn
            .call_ctap2_raw(0x06, &payload)
            .expect("raw ClientPin transport should succeed");
        let map = if status == 0 && !body.is_empty() {
            serde_cbor::from_slice(&body).expect("decode ClientPin response map")
        } else {
            std::collections::BTreeMap::new()
        };
        (status, map)
    }

    fn key_agreement_value(public_key: &PublicKey) -> Value {
        let encoded = public_key.to_encoded_point(false);
        Value::Map(
            [
                (Value::Integer(1), Value::Integer(2)),
                (Value::Integer(3), Value::Integer(-25)),
                (Value::Integer(-1), Value::Integer(1)),
                (
                    Value::Integer(-2),
                    Value::Bytes(encoded.x().unwrap().to_vec()),
                ),
                (
                    Value::Integer(-3),
                    Value::Bytes(encoded.y().unwrap().to_vec()),
                ),
            ]
            .into_iter()
            .collect(),
        )
    }

    /// getKeyAgreement (0x02) then derive the protocol-2 shared secret.
    pub fn establish(authn: &mut dyn TestAuthenticator) -> SharedSecret {
        let (status, map) = call(
            authn,
            vec![
                (Value::Integer(1), Value::Integer(2)),
                (Value::Integer(2), Value::Integer(SUB_GET_KEY_AGREEMENT)),
            ],
        );
        assert_eq!(status, 0, "getKeyAgreement(2) must succeed");
        let ka = match map.get(&Value::Integer(1)) {
            Some(Value::Map(m)) => m,
            other => panic!("missing keyAgreement in response: {:?}", other),
        };
        let x = match ka.get(&Value::Integer(-2)) {
            Some(Value::Bytes(b)) => b.clone(),
            other => panic!("keyAgreement missing x: {:?}", other),
        };
        let y = match ka.get(&Value::Integer(-3)) {
            Some(Value::Bytes(b)) => b.clone(),
            other => panic!("keyAgreement missing y: {:?}", other),
        };

        let mut sec1 = [0u8; 65];
        sec1[0] = 0x04;
        sec1[1..33].copy_from_slice(&x);
        sec1[33..65].copy_from_slice(&y);
        let peer = PublicKey::from_sec1_bytes(&sec1).expect("valid authenticator key");

        let secret_key = SecretKey::random(&mut OsRng);
        let platform_public = secret_key.public_key();
        let shared = diffie_hellman(secret_key.to_nonzero_scalar(), peer.as_affine());
        let ikm = shared.raw_secret_bytes();

        let salt = [0u8; 32];
        let hmac_key = hkdf_sha256_32(&salt, ikm.as_slice(), b"CTAP2 HMAC key");
        let aes_key = hkdf_sha256_32(&salt, ikm.as_slice(), b"CTAP2 AES key");

        SharedSecret {
            hmac_key,
            aes_key,
            platform_public,
        }
    }

    impl SharedSecret {
        /// AES-256-CBC with a fresh random IV prepended.
        pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
            let mut iv = [0u8; 16];
            OsRng.fill_bytes(&mut iv);
            let mut buffer = data.to_vec();
            let len = buffer.len();
            Aes256CbcEnc::new_from_slices(&self.aes_key, &iv)
                .unwrap()
                .encrypt_padded_mut::<NoPadding>(&mut buffer, len)
                .unwrap();
            let mut out = Vec::with_capacity(16 + buffer.len());
            out.extend_from_slice(&iv);
            out.extend_from_slice(&buffer);
            out
        }

        pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
            let (iv, ct) = data.split_at(16);
            let mut buffer = ct.to_vec();
            Aes256CbcDec::new_from_slices(&self.aes_key, iv)
                .unwrap()
                .decrypt_padded_mut::<NoPadding>(&mut buffer)
                .unwrap()
                .to_vec()
        }

        /// Full 32-byte HMAC-SHA-256 under the shared HMAC key.
        pub fn authenticate(&self, data: &[u8]) -> [u8; 32] {
            let mut mac = HmacSha256::new_from_slice(&self.hmac_key).unwrap();
            mac.update(data);
            mac.finalize().into_bytes().into()
        }

        pub fn key_agreement(&self) -> Value {
            key_agreement_value(&self.platform_public)
        }
    }

    fn pin_hash_left16(pin: &str) -> Vec<u8> {
        Sha256::digest(pin.as_bytes())[..16].to_vec()
    }

    fn padded_pin(pin: &str) -> Vec<u8> {
        let mut buf = vec![0u8; 64];
        buf[..pin.len()].copy_from_slice(pin.as_bytes());
        buf
    }

    /// getRetries (0x01) — returns the pinRetries field.
    pub fn get_retries(authn: &mut dyn TestAuthenticator) -> u8 {
        let (status, map) = call(
            authn,
            vec![
                (Value::Integer(1), Value::Integer(2)),
                (Value::Integer(2), Value::Integer(SUB_GET_RETRIES)),
            ],
        );
        assert_eq!(status, 0, "getRetries(2) must succeed");
        match map.get(&Value::Integer(3)) {
            Some(Value::Integer(n)) => *n as u8,
            other => panic!("missing pinRetries: {:?}", other),
        }
    }

    /// setPIN (0x03). Returns the status byte.
    pub fn set_pin(authn: &mut dyn TestAuthenticator, pin: &str) -> u8 {
        let ss = establish(authn);
        let new_pin_enc = ss.encrypt(&padded_pin(pin));
        let pin_auth = ss.authenticate(&new_pin_enc);
        let (status, _) = call(
            authn,
            vec![
                (Value::Integer(1), Value::Integer(2)),
                (Value::Integer(2), Value::Integer(SUB_SET_PIN)),
                (Value::Integer(3), ss.key_agreement()),
                (Value::Integer(4), Value::Bytes(pin_auth.to_vec())),
                (Value::Integer(5), Value::Bytes(new_pin_enc)),
            ],
        );
        status
    }

    /// setPIN over an already-derived shared secret with an explicit plaintext
    /// buffer (used for policy-violation cases that send malformed PIN buffers).
    pub fn set_pin_raw(
        authn: &mut dyn TestAuthenticator,
        ss: &SharedSecret,
        plaintext: &[u8],
    ) -> u8 {
        let new_pin_enc = ss.encrypt(plaintext);
        let pin_auth = ss.authenticate(&new_pin_enc);
        let (status, _) = call(
            authn,
            vec![
                (Value::Integer(1), Value::Integer(2)),
                (Value::Integer(2), Value::Integer(SUB_SET_PIN)),
                (Value::Integer(3), ss.key_agreement()),
                (Value::Integer(4), Value::Bytes(pin_auth.to_vec())),
                (Value::Integer(5), Value::Bytes(new_pin_enc)),
            ],
        );
        status
    }

    /// changePIN (0x04). Returns the status byte.
    pub fn change_pin(authn: &mut dyn TestAuthenticator, old_pin: &str, new_pin: &str) -> u8 {
        let ss = establish(authn);
        let pin_hash_enc = ss.encrypt(&pin_hash_left16(old_pin));
        let new_pin_enc = ss.encrypt(&padded_pin(new_pin));
        let mut auth_input = new_pin_enc.clone();
        auth_input.extend_from_slice(&pin_hash_enc);
        let pin_auth = ss.authenticate(&auth_input);
        let (status, _) = call(
            authn,
            vec![
                (Value::Integer(1), Value::Integer(2)),
                (Value::Integer(2), Value::Integer(SUB_CHANGE_PIN)),
                (Value::Integer(3), ss.key_agreement()),
                (Value::Integer(4), Value::Bytes(pin_auth.to_vec())),
                (Value::Integer(5), Value::Bytes(new_pin_enc)),
                (Value::Integer(6), Value::Bytes(pin_hash_enc)),
            ],
        );
        status
    }

    /// A decrypted protocol-2 pinUvAuthToken (32 bytes).
    pub struct PinToken(pub Vec<u8>);

    impl PinToken {
        pub fn authenticate(&self, data: &[u8]) -> [u8; 32] {
            let mut mac = HmacSha256::new_from_slice(&self.0).unwrap();
            mac.update(data);
            mac.finalize().into_bytes().into()
        }
    }

    /// getPinToken (0x05). Returns `(status, Option<PinToken>)`.
    pub fn get_pin_token(authn: &mut dyn TestAuthenticator, pin: &str) -> (u8, Option<PinToken>) {
        let ss = establish(authn);
        let pin_hash_enc = ss.encrypt(&pin_hash_left16(pin));
        let (status, map) = call(
            authn,
            vec![
                (Value::Integer(1), Value::Integer(2)),
                (Value::Integer(2), Value::Integer(SUB_GET_PIN_TOKEN)),
                (Value::Integer(3), ss.key_agreement()),
                (Value::Integer(6), Value::Bytes(pin_hash_enc)),
            ],
        );
        if status != 0 {
            return (status, None);
        }
        let enc = match map.get(&Value::Integer(2)) {
            Some(Value::Bytes(b)) => b.clone(),
            other => panic!("missing pinUvAuthToken: {:?}", other),
        };
        (status, Some(PinToken(ss.decrypt(&enc))))
    }

    /// getPinUvAuthTokenUsingPinWithPermissions (0x09).
    /// Returns `(status, Option<PinToken>)`.
    pub fn get_pin_token_with_permissions(
        authn: &mut dyn TestAuthenticator,
        pin: &str,
        permissions: u8,
        rp_id: Option<&str>,
    ) -> (u8, Option<PinToken>) {
        let ss = establish(authn);
        let pin_hash_enc = ss.encrypt(&pin_hash_left16(pin));
        let mut entries = vec![
            (Value::Integer(1), Value::Integer(2)),
            (
                Value::Integer(2),
                Value::Integer(SUB_GET_TOKEN_USING_PIN_PERMS),
            ),
            (Value::Integer(3), ss.key_agreement()),
            (Value::Integer(6), Value::Bytes(pin_hash_enc)),
            (Value::Integer(9), Value::Integer(permissions as i128)),
        ];
        if let Some(rp_id) = rp_id {
            entries.push((Value::Integer(0x0A), Value::Text(rp_id.to_string())));
        }
        let (status, map) = call(authn, entries);
        if status != 0 {
            return (status, None);
        }
        let enc = match map.get(&Value::Integer(2)) {
            Some(Value::Bytes(b)) => b.clone(),
            other => panic!("missing pinUvAuthToken: {:?}", other),
        };
        (status, Some(PinToken(ss.decrypt(&enc))))
    }
}

/// ClientPin2-KeyAgreement P-1: getKeyAgreement under PIN protocol 2 returns a
/// COSE_Key with kty=EC2 (2), alg=ECDH-ES+HKDF-256 (-25), crv=P-256 (1) and
/// 32-byte x/y coordinates — and no other coefficients.
#[test]
#[serial]
fn pin2_get_key_agreement_fields() {
    run_in_thread(|| {
        with_authenticator!(pin2_get_key_agreement_fields, |authn| {
            let payload = support::raw::encode(&Value::Map(
                [
                    (Value::Integer(1), Value::Integer(2)),
                    (Value::Integer(2), Value::Integer(0x02)),
                ]
                .into_iter()
                .collect(),
            ));
            let (status, body) = authn
                .call_ctap2_raw(0x06, &payload)
                .expect("raw getKeyAgreement(2) should succeed");
            assert_eq!(status, 0, "getKeyAgreement(2) must succeed");
            let map: std::collections::BTreeMap<Value, Value> =
                serde_cbor::from_slice(&body).expect("decode response");
            let ka = match map.get(&Value::Integer(1)) {
                Some(Value::Map(m)) => m,
                other => panic!("missing keyAgreement: {:?}", other),
            };
            assert_eq!(
                ka.get(&Value::Integer(1)),
                Some(&Value::Integer(2)),
                "kty=EC2"
            );
            assert_eq!(
                ka.get(&Value::Integer(3)),
                Some(&Value::Integer(-25)),
                "alg=ECDH-ES+HKDF-256"
            );
            if let Some(crv) = ka.get(&Value::Integer(-1)) {
                assert_eq!(crv, &Value::Integer(1), "crv=P-256");
            }
            match ka.get(&Value::Integer(-2)) {
                Some(Value::Bytes(x)) => assert_eq!(x.len(), 32, "x must be 32 bytes"),
                other => panic!("missing x: {:?}", other),
            }
            match ka.get(&Value::Integer(-3)) {
                Some(Value::Bytes(y)) => assert_eq!(y.len(), 32, "y must be 32 bytes"),
                other => panic!("missing y: {:?}", other),
            }
            // Only kty(1), alg(3), crv(-1), x(-2), y(-3) are allowed.
            for key in ka.keys() {
                match key {
                    Value::Integer(1)
                    | Value::Integer(3)
                    | Value::Integer(-1)
                    | Value::Integer(-2)
                    | Value::Integer(-3) => {}
                    other => panic!("unexpected COSE coefficient: {:?}", other),
                }
            }
        })
    });
}

/// ClientPin2-NewPin / GetPinToken P-1: under PIN protocol 2, setPIN then
/// getPinToken succeed end-to-end (shared-secret derivation, IV-prefixed
/// AES-256-CBC, full-length pinUvAuthParam, token decryption).
#[test]
#[serial]
fn pin2_set_pin_and_get_token() {
    run_isolated_in_sim("pin::pin2_set_pin_and_get_token", || {
        run_in_thread(|| {
            with_authenticator!(pin2_set_pin_and_get_token, |authn| {
                reset_authenticator(authn);
                assert_eq!(p2::set_pin(authn, PIN1), 0, "setPIN(2) must succeed");

                let (status, token) = p2::get_pin_token(authn, PIN1);
                assert_eq!(status, 0, "getPinToken(2) must succeed");
                let token = token.expect("token present");
                assert_eq!(token.0.len(), 32, "protocol-2 pinUvAuthToken is 32 bytes");
            })
        });
    });
}

/// ClientPin2-GetPinToken P-2: a MakeCredential carrying a protocol-2
/// pinUvAuthParam (full 32-byte HMAC over the clientDataHash) succeeds and sets
/// the UV flag in authData.
#[test]
#[serial]
fn pin2_make_credential_sets_uv_flag() {
    run_isolated_in_sim("pin::pin2_make_credential_sets_uv_flag", || {
        run_in_thread(|| {
            with_authenticator!(pin2_make_credential_sets_uv_flag, |authn| {
                reset_authenticator(authn);
                assert_eq!(p2::set_pin(authn, PIN1), 0, "setPIN(2) must succeed");

                let (status, token) = p2::get_pin_token(authn, PIN1);
                assert_eq!(status, 0);
                let token = token.expect("token present");

                let mut request = make_credential_request();
                request.pin_protocol = Some(2);
                let pin_auth = token.authenticate(request.client_data_hash.as_ref());
                request.pin_auth = Some(leak_bytes(pin_auth.to_vec()));

                up::approve();
                let mc = match authn
                    .call_ctap2(&Request::MakeCredential(request))
                    .expect("MC with protocol-2 pinAuth should succeed")
                {
                    Response::MakeCredential(mc) => mc,
                    other => panic!("Expected MakeCredential, got {:?}", other),
                };
                assert!(
                    mc.auth_data[32] & (1 << 2) != 0,
                    "MC with protocol-2 pinAuth must set UV bit",
                );
            })
        });
    });
}

/// ClientPin2-GetPinToken P-3: a GetAssertion carrying a protocol-2
/// pinUvAuthParam succeeds and sets the UV flag in authData.
#[test]
#[serial]
fn pin2_get_assertion_sets_uv_flag() {
    run_isolated_in_sim("pin::pin2_get_assertion_sets_uv_flag", || {
        run_in_thread(|| {
            with_authenticator!(pin2_get_assertion_sets_uv_flag, |authn| {
                reset_authenticator(authn);
                assert_eq!(p2::set_pin(authn, PIN1), 0, "setPIN(2) must succeed");

                // Register a credential under a protocol-2 token.
                let (status, token) = p2::get_pin_token(authn, PIN1);
                assert_eq!(status, 0);
                let token = token.expect("token present");
                let mut mc_req = make_credential_request();
                mc_req.pin_protocol = Some(2);
                let mc_auth = token.authenticate(mc_req.client_data_hash.as_ref());
                mc_req.pin_auth = Some(leak_bytes(mc_auth.to_vec()));
                up::approve();
                let credential_id = match authn
                    .call_ctap2(&Request::MakeCredential(mc_req))
                    .expect("MC should succeed")
                {
                    Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                    other => panic!("Expected MakeCredential, got {:?}", other),
                };

                // Fresh token for the assertion.
                let (status, token) = p2::get_pin_token(authn, PIN1);
                assert_eq!(status, 0);
                let token = token.expect("token present");
                let mut ga_req = get_assertion_request(&credential_id);
                ga_req.pin_protocol = Some(2);
                let ga_auth = token.authenticate(ga_req.client_data_hash.as_ref());
                ga_req.pin_auth = Some(leak_bytes(ga_auth.to_vec()));

                up::approve();
                let ga = match authn
                    .call_ctap2(&Request::GetAssertion(ga_req))
                    .expect("GA with protocol-2 pinAuth should succeed")
                {
                    Response::GetAssertion(ga) => ga,
                    other => panic!("Expected GetAssertion, got {:?}", other),
                };
                assert_ne!(
                    ga.auth_data[32] & (1 << 2),
                    0,
                    "GA with protocol-2 pinAuth must set UV bit",
                );
            })
        });
    });
}

/// ClientPin2-NewPin P-2: changePIN under protocol 2 succeeds; afterwards the
/// old PIN is rejected (PinInvalid) and the new PIN yields a token.
#[test]
#[serial]
fn pin2_change_pin_updates_active_pin() {
    run_isolated_in_sim("pin::pin2_change_pin_updates_active_pin", || {
        run_in_thread(|| {
            with_authenticator!(pin2_change_pin_updates_active_pin, |authn| {
                reset_authenticator(authn);
                assert_eq!(p2::set_pin(authn, PIN1), 0, "setPIN(2) must succeed");
                assert_eq!(
                    p2::change_pin(authn, PIN1, PIN2),
                    0,
                    "changePIN(2) must succeed",
                );

                // Old PIN now invalid.
                let (status, _) = p2::get_pin_token(authn, PIN1);
                assert_eq!(
                    transport::error_from_byte(status),
                    ctap2::Error::PinInvalid,
                    "old PIN must be rejected after changePIN",
                );

                // New PIN works.
                let (status, token) = p2::get_pin_token(authn, PIN2);
                assert_eq!(status, 0, "getPinToken(2) with new PIN must succeed");
                assert!(token.is_some());
            })
        });
    });
}

/// ClientPin2-GetRetries P-3 (tail): under protocol 2, two failed getPinToken
/// attempts decrement retries by two, and a successful PIN-authenticated
/// MakeCredential resets the counter to the original value.
#[test]
#[serial]
fn pin2_retries_decrement_and_reset() {
    run_isolated_in_sim("pin::pin2_retries_decrement_and_reset", || {
        run_in_thread(|| {
            with_authenticator!(pin2_retries_decrement_and_reset, |authn| {
                reset_authenticator(authn);
                assert_eq!(p2::set_pin(authn, PIN1), 0, "setPIN(2) must succeed");

                let original = p2::get_retries(authn);
                assert_eq!(original, 8, "fresh PIN should start at 8 retries");

                for expected_after in [7u8, 6u8] {
                    let (status, _) = p2::get_pin_token(authn, "wrong-pin");
                    assert_eq!(transport::error_from_byte(status), ctap2::Error::PinInvalid,);
                    assert_eq!(p2::get_retries(authn), expected_after);
                }

                // Successful auth resets retries.
                let (status, token) = p2::get_pin_token(authn, PIN1);
                assert_eq!(status, 0);
                let token = token.expect("token present");
                let mut request = make_credential_request();
                request.pin_protocol = Some(2);
                let pin_auth = token.authenticate(request.client_data_hash.as_ref());
                request.pin_auth = Some(leak_bytes(pin_auth.to_vec()));
                up::approve();
                let _ = authn
                    .call_ctap2(&Request::MakeCredential(request))
                    .expect("MC with protocol-2 pinAuth should succeed");

                assert_eq!(
                    p2::get_retries(authn),
                    original,
                    "retries must reset after a successful protocol-2 PIN auth",
                );
            })
        });
    });
}

/// ClientPin2-GetRetries P-4: three consecutive bad getPinToken attempts
/// escalate to CTAP2_ERR_PIN_AUTH_BLOCKED on the third (per-boot batch of 3).
#[test]
#[serial]
fn pin2_three_bad_attempts_block_pin_auth() {
    run_isolated_in_sim("pin::pin2_three_bad_attempts_block_pin_auth", || {
        run_in_thread(|| {
            with_authenticator!(pin2_three_bad_attempts_block_pin_auth, |authn| {
                reset_authenticator(authn);
                assert_eq!(p2::set_pin(authn, PIN1), 0, "setPIN(2) must succeed");

                let expected = [
                    ctap2::Error::PinInvalid,
                    ctap2::Error::PinInvalid,
                    ctap2::Error::PinAuthBlocked,
                ];
                for want in expected {
                    let (status, _) = p2::get_pin_token(authn, "wrong-pin");
                    assert_eq!(transport::error_from_byte(status), want);
                }
            })
        });
    });
}

/// ClientPin2-PinPolicy F-1: under protocol 2, setting a PIN shorter than the
/// 4-codepoint minimum fails with CTAP2_ERR_PIN_POLICY_VIOLATION.
#[test]
#[serial]
fn pin2_set_pin_too_short_rejected() {
    run_isolated_in_sim("pin::pin2_set_pin_too_short_rejected", || {
        run_in_thread(|| {
            with_authenticator!(pin2_set_pin_too_short_rejected, |authn| {
                reset_authenticator(authn);
                let ss = p2::establish(authn);
                // 3-byte PIN in a 64-byte zero buffer.
                let mut plaintext = vec![0u8; 64];
                plaintext[..3].copy_from_slice(b"123");
                let status = p2::set_pin_raw(authn, &ss, &plaintext);
                assert_eq!(
                    transport::error_from_byte(status),
                    ctap2::Error::PinPolicyViolation,
                );
            })
        });
    });
}

/// ClientPin2-PinPolicy F-3: under protocol 2, setting a PIN of exactly 64
/// bytes (no NUL terminator within the 64-byte buffer) fails with
/// CTAP2_ERR_PIN_POLICY_VIOLATION.
#[test]
#[serial]
fn pin2_set_pin_too_long_rejected() {
    run_isolated_in_sim("pin::pin2_set_pin_too_long_rejected", || {
        run_in_thread(|| {
            with_authenticator!(pin2_set_pin_too_long_rejected, |authn| {
                reset_authenticator(authn);
                let ss = p2::establish(authn);
                // 64 non-zero bytes: the decrypted PIN has no NUL within 64,
                // so its codepoint length exceeds the 63-byte maximum.
                let plaintext = vec![0x41u8; 64];
                let status = p2::set_pin_raw(authn, &ss, &plaintext);
                assert_eq!(
                    transport::error_from_byte(status),
                    ctap2::Error::PinPolicyViolation,
                );
            })
        });
    });
}

/// ClientPin2-GetPinUvAuthTokenUsingPinWithPermissions P-1/P-2: acquire a
/// protocol-2 token with mc|ga permissions bound to an RP ID, then use it to
/// register a credential (UV flag set).
#[test]
#[serial]
fn pin2_token_with_permissions_make_credential() {
    run_isolated_in_sim("pin::pin2_token_with_permissions_make_credential", || {
        run_in_thread(|| {
            with_authenticator!(pin2_token_with_permissions_make_credential, |authn| {
                reset_authenticator(authn);
                assert_eq!(p2::set_pin(authn, PIN1), 0, "setPIN(2) must succeed");

                // mc(0x01) | ga(0x02), bound to the RP we register under.
                let (status, token) = p2::get_pin_token_with_permissions(
                    authn,
                    PIN1,
                    0x01 | 0x02,
                    Some("example.com"),
                );
                assert_eq!(
                    status, 0,
                    "getPinUvAuthTokenUsingPinWithPermissions(2) must succeed",
                );
                let token = token.expect("token present");

                let mut request = make_credential_request();
                request.pin_protocol = Some(2);
                let pin_auth = token.authenticate(request.client_data_hash.as_ref());
                request.pin_auth = Some(leak_bytes(pin_auth.to_vec()));

                up::approve();
                let mc = match authn
                    .call_ctap2(&Request::MakeCredential(request))
                    .expect("MC with protocol-2 permissioned token should succeed")
                {
                    Response::MakeCredential(mc) => mc,
                    other => panic!("Expected MakeCredential, got {:?}", other),
                };
                assert!(
                    mc.auth_data[32] & (1 << 2) != 0,
                    "MC under a permissioned protocol-2 token must set UV bit",
                );
            })
        });
    });
}
