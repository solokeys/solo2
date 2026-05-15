//! pinComplexityPolicy extension tests (CTAP 2.3).
//!
//! Ports `tests/CTAP2/Protocol/Extensions/pinComplexityPolicy.js` from the FIDO
//! CTAP2.3 conformance module.
//!
//! The conformance suite's `before()` hook reads GetInfo and, if the
//! `pinComplexityPolicy` extension is NOT advertised, calls `this.skip()` to
//! skip the entire suite (both P-1 and P-2). Our device does not advertise the
//! `pinComplexityPolicy` extension (its advertised extensions are credProtect,
//! credBlob, hmac-secret, hmac-secret-mc, largeBlobKey, minPinLength,
//! thirdPartyPayment), so the positive cases P-1 and P-2 are not applicable.
//!
//! We therefore port the suite as a single GetInfo-level "device does not
//! support pinComplexityPolicy" assertion, mirroring the conformance skip
//! condition:
//!   - the GetInfo `extensions` list must NOT contain "pinComplexityPolicy";
//!   - the GetInfo `pinComplexityPolicy` member (0x1B) must be absent
//!     (per the spec, 0x1B is only present when the extension is supported).

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

/// Conformance `before()` skip condition, asserted as device-not-supported.
///
/// The `pinComplexityPolicy` extension is not advertised by our device, so:
///   - it must be absent from the GetInfo `extensions` array, and
///   - the optional GetInfo `pinComplexityPolicy` member (0x1B) must be `None`.
///
/// If a future build adds the extension, this test flips to a real
/// not-implemented signal so the positive cases (P-1, P-2) can be ported.
#[test]
#[serial]
fn pin_complexity_policy_not_supported() {
    run_in_thread(|| {
        with_authenticator!(pin_complexity_policy, |authn| {
            reset_authenticator(authn);

            let info = get_info(authn);

            // The `Extension` enum in ctap-types has no `pinComplexityPolicy`
            // variant, so any advertised extension is necessarily not it.
            // Assert via the serialized wire name to stay robust if a variant
            // is added later.
            let advertised: &[ctap2::get_info::Extension] =
                info.extensions.as_deref().unwrap_or(&[]);
            for ext in advertised {
                let name: &str = (*ext).into();
                assert_ne!(
                    name, "pinComplexityPolicy",
                    "device must not advertise the pinComplexityPolicy extension"
                );
            }

            // Per CTAP 2.3, the GetInfo `pinComplexityPolicy` member (0x1B) is
            // only present when the extension is supported. It must be absent.
            assert!(
                info.pin_complexity_policy.is_none(),
                "pinComplexityPolicy (0x1B) must be absent when the extension \
                 is unsupported, got {:?}",
                info.pin_complexity_policy,
            );
        })
    });
}
