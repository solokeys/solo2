//! Reset and reboot tests.

use super::*;

#[test]
#[serial]
fn reset_group() {
    run_in_thread(|| {
        // --- basic reset ---
        with_authenticator!(reset1, Conforming {}, |authn| {
            reset_authenticator(authn);
        });

        // --- reset invalidates credentials ---
        with_authenticator!(reset2, Conforming {}, |authn| {
            reset_authenticator(authn);
            up::approve_sticky();
            let mut req = make_credential_request();
            req.options = Some(decode_from_value(options_value(Some(true), None, None)));
            authn
                .call_ctap2(&Request::MakeCredential(req))
                .expect("MC should succeed");

            // Now reset again
            reset_authenticator(authn);

            // Credential should be gone
            up::approve();
            let ga = get_assertion_request_for("example.com", None);
            assert!(
                authn.call_ctap2(&Request::GetAssertion(ga)).is_err(),
                "credential should be gone"
            );
        });
    });
}

/// Reboot persistence — in-process only.
///
/// IGNORED: original test built two separate `Service` instances over the same
/// leaked storage buffers using the pre-0.2 trussed `store!` / `platform!` /
/// `ClientImplementation::new(req, &mut svc)` API. The current trussed API uses
/// a service-thread + `Syscall` impl; restarting a fresh service against the
/// same static storage requires a new harness in `support/sim.rs`. Not yet
/// ported.
#[test]
#[serial]
#[ignore = "needs sim harness extension: re-mount RAM fs across two services"]
fn reboot_persistence() {}
