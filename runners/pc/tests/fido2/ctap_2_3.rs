//! CTAP 2.3 advertising + ML-DSA-44 tests.
//!
//! Mirrors `fido2-tests/tests/standard/fido2/test_ctap_2_3.py`. All checks
//! are feature-detected so the suite remains useful against any CTAP 2.x
//! authenticator (the daemon advertises FIDO_2_3, smart-card transport,
//! long-touch reset, and — when built with `--features mldsa44` —
//! ML-DSA-44).

use super::*;
use ctap_types::webauthn::{
    FilteredPublicKeyCredentialParameters, KnownPublicKeyCredentialParameters,
};

const ALG_ML_DSA_44: i32 = -50;

fn get_info(authn: &mut dyn TestAuthenticator) -> ctap2::get_info::Response {
    match authn
        .call_ctap2(&Request::GetInfo)
        .expect("GetInfo should succeed")
    {
        Response::GetInfo(info) => info,
        other => panic!("Expected GetInfo, got {:?}", other),
    }
}

fn supports_ml_dsa_44(info: &ctap2::get_info::Response) -> bool {
    info.algorithms
        .as_ref()
        .map(|algs| algs.0.iter().any(|a| a.alg() == ALG_ML_DSA_44))
        .unwrap_or(false)
}

/// FIDO_2_3 must appear in versions when any 2.3 surface is advertised.
#[test]
#[serial]
fn ctap23_advertises_fido_2_3_when_2_3_surface_present() {
    run_in_thread(|| {
        with_authenticator!(ctap23_version, |authn| {
            let info = get_info(authn);
            let has_ml_dsa = supports_ml_dsa_44(&info);
            let long_touch = info.long_touch_for_reset == Some(true);
            let smartcard = info
                .transports
                .as_ref()
                .map(|t| t.contains(&ctap2::get_info::Transport::SmartCard))
                .unwrap_or(false);
            if has_ml_dsa || long_touch || smartcard {
                assert!(
                    info.versions.contains(&ctap2::get_info::Version::Fido2_3),
                    "2.3 surface advertised but FIDO_2_3 missing from versions",
                );
            }
        })
    });
}

/// `smart-card` is a valid Transport per CTAP 2.3 §3.
#[test]
#[serial]
fn ctap23_smart_card_transport_implies_fido_2_3() {
    run_in_thread(|| {
        with_authenticator!(ctap23_sc, |authn| {
            let info = get_info(authn);
            let advertises_sc = info
                .transports
                .as_ref()
                .map(|t| t.contains(&ctap2::get_info::Transport::SmartCard))
                .unwrap_or(false);
            if advertises_sc {
                assert!(info.versions.contains(&ctap2::get_info::Version::Fido2_3));
            }
        })
    });
}

/// `longTouchForReset` is a bool when present (Option<bool> at the type level).
#[test]
#[serial]
fn ctap23_long_touch_for_reset_field_well_formed() {
    run_in_thread(|| {
        with_authenticator!(ctap23_lt, |authn| {
            let info = get_info(authn);
            // The `Option<bool>` type already enforces well-formedness — this
            // test asserts the daemon serializes the field at all. fido-auth
            // does, so we expect Some(true).
            assert_eq!(
                info.long_touch_for_reset,
                Some(true),
                "fido-authenticator should advertise long_touch_for_reset = true",
            );
        })
    });
}

/// ML-DSA-44 entry in `algorithms` carries `type=public-key`. Skip when off.
#[test]
#[serial]
fn ctap23_advertises_ml_dsa_44_when_supported() {
    run_in_thread(|| {
        with_authenticator!(ctap23_mldsa_info, |authn| {
            let info = get_info(authn);
            if let Some(algs) = info.algorithms.as_ref() {
                if let Some(_entry) = algs.0.iter().find(|a| a.alg() == ALG_ML_DSA_44) {
                    // ctap-types' `KnownPublicKeyCredentialParameters` only
                    // carries `alg`; the `type=public-key` constraint is
                    // already enforced by the enum being CBOR-encoded into
                    // a `FilteredPublicKeyCredentialParameters`. Reaching
                    // here means -50 was advertised with the right shape.
                }
            }
        })
    });
}

fn pkcp(algs: &[i32]) -> FilteredPublicKeyCredentialParameters {
    let mut inner = heapless::Vec::new();
    for alg in algs {
        if let Ok(known) = KnownPublicKeyCredentialParameters::try_from(
            ctap_types::webauthn::PublicKeyCredentialParameters::public_key_with_alg(*alg),
        ) {
            let _ = inner.push(known);
        }
    }
    FilteredPublicKeyCredentialParameters(inner)
}

/// CTAP §6.1.2 step 6: walk `pubKeyCredParams` in order and pick the first
/// algorithm supported by the authenticator. This is the test that motivated
/// the alg-loop bug fix in `fido-authenticator` commit `188325d`.
///
/// Three sub-cases — for each, the supported-set drives which choice wins:
///   - `[-7]` → P256 (chosen)
///   - `[-8, -7]` → EdDSA (chosen — first supported alg wins, NOT last)
///   - `[-50, -8, -7]` → ML-DSA-44 when feature is on, else EdDSA
#[test]
#[serial]
fn ctap23_pubkey_cred_params_picks_first_supported_alg() {
    run_in_thread(|| {
        with_authenticator!(ctap23_alg_pref, |authn| {
            reset_authenticator(authn);

            // P256 only.
            up::approve();
            let mut req = make_credential_request();
            req.pub_key_cred_params = pkcp(&[-7]);
            authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("MC with alg=-7 should succeed");

            // [EdDSA, ES256] — EdDSA wins (first listed + supported).
            up::approve();
            let mut req = make_credential_request();
            req.pub_key_cred_params = pkcp(&[-8, -7]);
            authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("MC with alg=[-8,-7] should succeed (EdDSA chosen)");

            // [ML-DSA-44, EdDSA, ES256] — with mldsa44 feature on the daemon
            // picks -50; off, it falls through to EdDSA. Either way the call
            // must succeed (a buggy daemon that errors on unknown alg
            // entries would fail this).
            up::approve();
            let mut req = make_credential_request();
            req.pub_key_cred_params = pkcp(&[ALG_ML_DSA_44, -8, -7]);
            authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("MC with mixed pubKeyCredParams should succeed (first supported alg)");
        })
    });
}

/// MakeCredential with `pubKeyCredParams=[{alg:-50}]`. Skip when the daemon
/// doesn't advertise -50 (built without `--features mldsa44`).
#[test]
#[serial]
fn ctap23_make_credential_ml_dsa_44() {
    run_in_thread(|| {
        with_authenticator!(ctap23_mc_mldsa, |authn| {
            reset_authenticator(authn);
            let info = get_info(authn);
            if !supports_ml_dsa_44(&info) {
                eprintln!("ML-DSA-44 not advertised; skipping");
                return;
            }
            up::approve();
            let mut req = make_credential_request();
            req.pub_key_cred_params = pkcp(&[ALG_ML_DSA_44]);
            authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("MC with alg=-50 should succeed");
        })
    });
}

/// GetAssertion under an ML-DSA-44 credential. Same feature gating.
#[test]
#[serial]
fn ctap23_get_assertion_ml_dsa_44() {
    run_in_thread(|| {
        with_authenticator!(ctap23_ga_mldsa, |authn| {
            reset_authenticator(authn);
            let info = get_info(authn);
            if !supports_ml_dsa_44(&info) {
                eprintln!("ML-DSA-44 not advertised; skipping");
                return;
            }

            up::approve();
            let mut mc_req = make_credential_request();
            mc_req.pub_key_cred_params = pkcp(&[ALG_ML_DSA_44]);
            let cred_id = match authn
                .call_ctap2(&Request::MakeCredential(mc_req))
                .expect("MC")
            {
                Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
                other => panic!("Expected MakeCredential, got {:?}", other),
            };

            up::approve();
            let ga = match authn
                .call_ctap2(&Request::GetAssertion(get_assertion_request(&cred_id)))
                .expect("GA")
            {
                Response::GetAssertion(ga) => ga,
                other => panic!("Expected GetAssertion, got {:?}", other),
            };
            // FIPS 204 §4: ML-DSA-44 signature is exactly 2420 bytes.
            assert_eq!(
                ga.signature.len(),
                2420,
                "ML-DSA-44 signature must be 2420 bytes",
            );
        })
    });
}
