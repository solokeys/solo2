//! User presence tests — approve/deny via simulated hardware buttons (`solo_pc::buttons`).
//!
//! Denied UP waits for fido-authenticator's FIDO2 timeout (30 s of *logical* uptime). With the
//! default `test-fast-up-clock` feature, `solo_pc::UserInterface::uptime` is scaled so that
//! window elapses in a few hundred milliseconds of wall time while Trussed still polls
//! `check_user_presence` until timeout (see `solo_pc::user_presence_poll_count()`).

use super::*;

#[test]
#[serial]
fn up_group() {
    run_in_thread(|| {
        with_authenticator!(up_group, Conforming {}, |authn| {
            reset_authenticator(authn);

            // MC approved
            let polls_before = solo_pc::user_presence_poll_count();
            up::approve();
            let resp = authn
                .call_ctap2(&Request::MakeCredential(make_credential_request()))
                .expect("MC with UP should succeed");
            assert!(matches!(resp, Response::MakeCredential(_)));
            assert!(
                solo_pc::user_presence_poll_count() > polls_before,
                "MakeCredential must poll user presence (Conforming UP)"
            );

            // MC denied — Trussed busy-polls until timeout; polls should increase substantially.
            let polls_before_deny = solo_pc::user_presence_poll_count();
            up::deny();
            let result = authn.call_ctap2(&Request::MakeCredential(make_credential_request()));
            assert!(result.is_err(), "MC should fail when UP denied");
            assert!(
                solo_pc::user_presence_poll_count() > polls_before_deny,
                "denied MC must keep polling user presence until timeout"
            );
            up::reset();

            // GA approved
            let polls_before_ga = solo_pc::user_presence_poll_count();
            up::approve_sticky();
            let cred_id = make_credential(authn);
            let resp = authn
                .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                    "example.com",
                    Some(single_allow_list(&cred_id)),
                )))
                .expect("GA with UP should succeed");
            assert!(matches!(resp, Response::GetAssertion(_)));
            assert!(
                solo_pc::user_presence_poll_count() > polls_before_ga,
                "GetAssertion must poll user presence"
            );

            // GA denied
            let polls_before_ga_deny = solo_pc::user_presence_poll_count();
            up::deny();
            let result = authn.call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                "example.com",
                Some(single_allow_list(&cred_id)),
            )));
            assert!(result.is_err(), "GA should fail when UP denied");
            assert!(
                solo_pc::user_presence_poll_count() > polls_before_ga_deny,
                "denied GA must keep polling user presence until timeout"
            );
            up::reset();
        })
    });
}
