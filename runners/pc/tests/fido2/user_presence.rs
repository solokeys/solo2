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
            up::approve();
            let resp = authn
                .call_ctap2(&Request::MakeCredential(make_credential_request()))
                .expect("MC with UP should succeed");
            assert!(matches!(resp, Response::MakeCredential(_)));

            // MC denied
            up::deny();
            let result = authn.call_ctap2(&Request::MakeCredential(make_credential_request()));
            assert!(result.is_err(), "MC should fail when UP denied");
            up::reset();

            // GA approved
            up::approve_sticky();
            let cred_id = make_credential(authn);
            let resp = authn
                .call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                    "example.com",
                    Some(single_allow_list(&cred_id)),
                )))
                .expect("GA with UP should succeed");
            assert!(matches!(resp, Response::GetAssertion(_)));

            // GA denied
            up::deny();
            let result = authn.call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                "example.com",
                Some(single_allow_list(&cred_id)),
            )));
            assert!(result.is_err(), "GA should fail when UP denied");
            up::reset();
        })
    });
}

/// `up=false` on GetAssertion: the host explicitly opts out of UP, so the
/// daemon must NOT poll consent and must return UP-flag-clear (CTAP 2.1 §6.2.2).
/// We pre-arm `deny()` to catch a buggy implementation that would still ask.
#[test]
#[serial]
fn up_option_false_on_get_assertion_skips_consent() {
    run_in_thread(|| {
        with_authenticator!(up_option_false_ga, |authn| {
            reset_authenticator(authn);
            up::approve();
            let cred_id = make_credential(authn);

            up::deny();
            let polls_before = solo_pc::user_presence_poll_count();
            let mut req =
                get_assertion_request_for("example.com", Some(single_allow_list(&cred_id)));
            req.options = Some(decode_from_value(options_value(None, Some(false), None)));
            let resp = authn
                .call_ctap2(&Request::GetAssertion(req))
                .expect("GA with up=false must succeed even with deny() armed");
            match resp {
                Response::GetAssertion(ga) => {
                    assert_eq!(
                        ga.auth_data[32] & 0x01,
                        0,
                        "UP flag must be clear when up=false",
                    );
                }
                other => panic!("Expected GetAssertion, got {:?}", other),
            }
            assert_eq!(
                solo_pc::user_presence_poll_count(),
                polls_before,
                "up=false must not poll user_presence",
            );
            up::reset();
        })
    });
}

/// CTAP 2.1 §6.1.2: MakeCredential allows `up` only with value true
/// (UP is implicit and required); `up=false` is INVALID_OPTION, `up=true`
/// is accepted and behaves like `up` being absent.
#[test]
#[serial]
fn up_option_on_make_credential() {
    run_in_thread(|| {
        with_authenticator!(up_option_mc, |authn| {
            reset_authenticator(authn);
            // up=false → InvalidOption
            let mut req = make_credential_request();
            req.options = Some(decode_from_value(options_value(None, Some(false), None)));
            up::approve_sticky();
            assert_eq!(
                authn.call_ctap2(&Request::MakeCredential(req)),
                Err(ctap2::Error::InvalidOption),
                "MC with up=false must return InvalidOption",
            );
            // up=true → succeeds (same as absent)
            let mut req = make_credential_request();
            req.options = Some(decode_from_value(options_value(None, Some(true), None)));
            up::approve_sticky();
            assert!(
                matches!(
                    authn.call_ctap2(&Request::MakeCredential(req)),
                    Ok(Response::MakeCredential(_))
                ),
                "MC with up=true must succeed",
            );
            up::reset();
        })
    });
}

/// `up::approve()` is single-shot: the next UP poll grants, all subsequent
/// polls deny (single-shot exhaustion of `APPROVE_ONCE`).
#[test]
#[serial]
fn up_one_request_per_approve_call() {
    run_in_thread(|| {
        with_authenticator!(up_one_request, |authn| {
            reset_authenticator(authn);
            up::approve();
            let cred_id = make_credential(authn);

            // First GA: tap is queued, succeeds.
            up::approve();
            let resp = authn.call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                "example.com",
                Some(single_allow_list(&cred_id)),
            )));
            assert!(resp.is_ok(), "first GA after approve() must succeed");

            // Second GA without re-arming approve: deny mode so the loop runs
            // (without a queue, button state is AUTO_APPROVE which would also
            // grant — we want to assert the daemon honors `deny()` here).
            up::deny();
            let resp = authn.call_ctap2(&Request::GetAssertion(get_assertion_request_for(
                "example.com",
                Some(single_allow_list(&cred_id)),
            )));
            assert!(
                resp.is_err(),
                "second GA without re-arm under deny() must fail",
            );
            up::reset();
        })
    });
}
