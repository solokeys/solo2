//! thirdPartyPayment extension tests.
//!
//! Ported from the FIDO CTAP2.3 conformance module
//! `tests/CTAP2/Protocol/Extensions/thirdPartyPayment.js`.
//!
//! The `thirdPartyPayment` extension is set (boolean) at MakeCredential time
//! and bound to the credential. At GetAssertion time, requesting the extension
//! makes the authenticator echo a boolean in the authData extensions map:
//! `true` if the asserted credential was created with the extension, `false`
//! otherwise.
//!
//! Unlike `hmac-secret`, the extension output is a plain boolean carried in the
//! signed authData extensions CBOR map, so we parse it directly out of
//! `auth_data` rather than relying on a typed response field.

use super::*;

/// Parse the extensions CBOR map out of a GetAssertion `auth_data`.
///
/// GetAssertion authData has no attested credential data (no AT flag), so the
/// layout is: rpIdHash(32) || flags(1) || signCount(4) || extensions(CBOR map).
/// Returns the decoded map, asserting the ED (extension data, bit 7) flag is set.
fn ga_extensions_map(auth_data: &[u8]) -> serde_cbor::Value {
    assert!(
        auth_data.len() > 37,
        "auth_data too short to contain extensions: {} bytes",
        auth_data.len()
    );
    assert!(
        auth_data[32] & 0x80 != 0,
        "ED (extension data) flag must be set when thirdPartyPayment is requested"
    );
    // No AT flag for GetAssertion: extensions begin right after the 4-byte counter.
    let ext_bytes = &auth_data[37..];
    serde_cbor::from_slice(ext_bytes).expect("decode authData extensions CBOR map")
}

/// Extract the `thirdPartyPayment` boolean from a decoded extensions map.
fn third_party_payment_value(map: &serde_cbor::Value) -> Option<bool> {
    let serde_cbor::Value::Map(entries) = map else {
        panic!("authData extensions is not a CBOR map: {map:?}");
    };
    entries.iter().find_map(|(k, v)| match (k, v) {
        (serde_cbor::Value::Text(t), serde_cbor::Value::Bool(b)) if t == "thirdPartyPayment" => {
            Some(*b)
        }
        _ => None,
    })
}

/// MakeCredential, optionally requesting the `thirdPartyPayment` extension.
/// Returns the credential id.
fn mc(
    authn: &mut dyn TestAuthenticator,
    rp_id: &str,
    user_id: &[u8],
    rk: bool,
    third_party_payment: Option<bool>,
) -> Vec<u8> {
    let mut req = make_credential_request_for(rp_id, user_id, "tpp-user", rk);
    if let Some(tpp) = third_party_payment {
        let mut ext = ctap2::make_credential::ExtensionsInput::default();
        ext.third_party_payment = Some(tpp);
        req.extensions = Some(ext);
    }
    up::approve();
    match authn
        .call_ctap2(&Request::MakeCredential(req))
        .unwrap_or_else(|e| panic!("MakeCredential should succeed: {e:?}"))
    {
        Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
        other => panic!("Expected MakeCredential, got {:?}", other),
    }
}

/// GetAssertion against `cred_id`, requesting the `thirdPartyPayment` extension.
/// Returns the parsed `thirdPartyPayment` boolean from the authData extensions.
fn ga_third_party_payment(authn: &mut dyn TestAuthenticator, rp_id: &str, cred_id: &[u8]) -> bool {
    let mut req = get_assertion_request_for(rp_id, Some(single_allow_list(cred_id)));
    let mut ext = ctap2::get_assertion::ExtensionsInput::default();
    ext.third_party_payment = Some(true);
    req.extensions = Some(ext);
    up::approve();
    let ga = match authn
        .call_ctap2(&Request::GetAssertion(req))
        .unwrap_or_else(|e| panic!("GetAssertion should succeed: {e:?}"))
    {
        Response::GetAssertion(ga) => ga,
        other => panic!("Expected GetAssertion, got {:?}", other),
    };
    let map = ga_extensions_map(&ga.auth_data);
    third_party_payment_value(&map)
        .expect("authData extensions must contain a boolean thirdPartyPayment response")
}

/// P-1: discoverable (rk) credential created with thirdPartyPayment=true;
/// GetAssertion with thirdPartyPayment=true echoes boolean true.
#[test]
#[serial]
fn p1_discoverable_third_party_payment_true() {
    run_in_thread(|| {
        with_authenticator!(tpp_p1, |authn| {
            reset_authenticator(authn);

            let rp_id = "tpp-p1.example.com";
            let cred_id = mc(authn, rp_id, &[0x01; 16], true, Some(true));

            let tpp = ga_third_party_payment(authn, rp_id, &cred_id);
            assert!(
                tpp,
                "thirdPartyPayment must be TRUE for a credential created with the extension"
            );
        })
    });
}

/// P-2: non-discoverable (rk=false) credential created with
/// thirdPartyPayment=true; GetAssertion with thirdPartyPayment=true echoes
/// boolean true.
#[test]
#[serial]
fn p2_non_discoverable_third_party_payment_true() {
    run_in_thread(|| {
        with_authenticator!(tpp_p2, |authn| {
            reset_authenticator(authn);

            let rp_id = "tpp-p2.example.com";
            let cred_id = mc(authn, rp_id, &[0x02; 16], false, Some(true));

            let tpp = ga_third_party_payment(authn, rp_id, &cred_id);
            assert!(
                tpp,
                "thirdPartyPayment must be TRUE for a non-discoverable credential created with the extension"
            );
        })
    });
}

/// F-1: credential created WITHOUT the thirdPartyPayment extension;
/// GetAssertion with thirdPartyPayment=true echoes boolean false.
#[test]
#[serial]
fn f1_no_extension_third_party_payment_false() {
    run_in_thread(|| {
        with_authenticator!(tpp_f1, |authn| {
            reset_authenticator(authn);

            let rp_id = "tpp-f1.example.com";
            let cred_id = mc(authn, rp_id, &[0x03; 16], false, None);

            let tpp = ga_third_party_payment(authn, rp_id, &cred_id);
            assert!(
                !tpp,
                "thirdPartyPayment must be FALSE for a credential created without the extension"
            );
        })
    });
}

/// Info: the authenticator must advertise the `thirdPartyPayment` extension.
/// (The conformance `before` hook gates the whole suite on this; we assert it
/// explicitly so a regression in the advertised extension list is caught.)
#[test]
#[serial]
fn third_party_payment_in_info() {
    run_in_thread(|| {
        with_authenticator!(tpp_info, |authn| {
            let resp = authn.call_ctap2(&Request::GetInfo).expect("GetInfo");
            match resp {
                Response::GetInfo(info) => {
                    let exts = info.extensions.expect("extensions should be present");
                    assert!(
                        exts.contains(&ctap_types::ctap2::get_info::Extension::ThirdPartyPayment),
                        "thirdPartyPayment must be advertised"
                    );
                }
                other => panic!("Expected GetInfo, got {:?}", other),
            }
        })
    });
}
