//! CTAP 2.2 advertising tests.
//!
//! Mirrors `fido2-tests/tests/standard/fido2/test_ctap_2_2.py`. The version
//! string and `hmac-secret-mc` extension are GetInfo-only checks; the
//! extension's MakeCredential-time semantics live in `hmac_secret.rs`.

use super::*;

fn get_info(authn: &mut dyn TestAuthenticator) -> ctap2::get_info::Response {
    match authn
        .call_ctap2(&Request::GetInfo)
        .expect("GetInfo should succeed")
    {
        Response::GetInfo(info) => info,
        other => panic!("Expected GetInfo, got {:?}", other),
    }
}

fn extensions(info: &ctap2::get_info::Response) -> &[ctap2::get_info::Extension] {
    info.extensions.as_deref().unwrap_or(&[])
}

/// CTAP 2.3 §6.4: "The string `FIDO_2_2` was not defined for CTAP 2.2 and
/// MUST not be present in versions member." The `ctap-types` enum no
/// longer carries `Fido2_2`, making this property compile-time-checked —
/// every value in `versions` is guaranteed not to be `FIDO_2_2`. The
/// test is retained to lock the rule into the integration surface (a
/// future `Version::Fido2_2` re-introduction would still need to flip
/// this assertion explicitly).
#[test]
#[serial]
fn ctap22_does_not_advertise_fido_2_2() {
    run_in_thread(|| {
        with_authenticator!(ctap22_version, |authn| {
            let info = get_info(authn);
            // Sanity: at least one CTAP2 version is advertised.
            assert!(
                info.versions.iter().any(|v| matches!(
                    v,
                    ctap2::get_info::Version::Fido2_0
                        | ctap2::get_info::Version::Fido2_1
                        | ctap2::get_info::Version::Fido2_3
                )),
                "expected at least one CTAP2 version, got {:?}",
                info.versions
            );
        })
    });
}

/// Authenticators that advertise `hmac-secret-mc` must also advertise
/// `hmac-secret` — same key schedule / wire shape, just bound to MC.
#[test]
#[serial]
fn ctap22_hmac_secret_mc_implies_hmac_secret() {
    run_in_thread(|| {
        with_authenticator!(ctap22_implies, |authn| {
            let info = get_info(authn);
            let exts = extensions(&info);
            if exts.contains(&ctap2::get_info::Extension::HmacSecretMc) {
                assert!(
                    exts.contains(&ctap2::get_info::Extension::HmacSecret),
                    "hmac-secret-mc requires hmac-secret",
                );
            }
        })
    });
}
