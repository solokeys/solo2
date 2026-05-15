//! authenticatorConfig (0x0D) tests.
//!
//! Ported from the FIDO CTAP2.3 conformance module
//! (tests/CTAP2/Protocol/AuthenticatorConfig/AuthenticatorConfig.js +
//! js/AuthenticatorConfigUtils.js).
//!
//! Covered subcommands: toggleAlwaysUv (0x02) and setMinPINLength (0x03).
//! enableEnterpriseAttestation (0x01) is N/A for this device (no `ep` option),
//! and the conformance P-6 (pinComplexityPolicy) / P-7 (enableLongTouchForReset
//! advertised via getInfo.longTouchForReset) cases are skipped — see notes.
//!
//! authenticatorConfig requires a PIN to be set and a PinUvAuthToken carrying
//! the `acfg` (AUTHENTICATOR_CONFIGURATION = 0x20) permission. The pinUvAuthParam
//! authenticates over `0xff*32 || 0x0d || subCommand || subCommandParamsCBOR`.

use super::*;
use ctap_types::ctap2::client_pin::Permissions;
use ctap_types::ctap2::config::SubcommandParameters;
use serde_cbor::Value;
use support::pin::PinSession;

const PIN: &str = "123456";

const SUB_TOGGLE_ALWAYS_UV: u8 = 0x02;
const SUB_SET_MIN_PIN_LENGTH: u8 = 0x03;

/// Owned description of a `setMinPINLength` subcommand-params map.
#[derive(Default, Clone)]
struct ParamsSpec {
    new_min_pin_length: Option<u8>,
    min_pin_length_rp_ids: Option<Vec<String>>,
    force_change_pin: Option<bool>,
}

impl ParamsSpec {
    /// CBOR `Value` map with the indexed keys used on the wire (1/2/3).
    fn to_value(&self) -> Value {
        let mut entries = vec![];
        if let Some(v) = self.new_min_pin_length {
            entries.push((Value::Integer(1), Value::Integer(v as i128)));
        }
        if let Some(ids) = &self.min_pin_length_rp_ids {
            entries.push((
                Value::Integer(2),
                Value::Array(ids.iter().map(|s| Value::Text(s.clone())).collect()),
            ));
        }
        if let Some(b) = self.force_change_pin {
            entries.push((Value::Integer(3), Value::Bool(b)));
        }
        Value::Map(entries.into_iter().collect())
    }

    /// Re-serialize through the typed [`SubcommandParameters`] with `cbor_smol`,
    /// matching byte-for-byte what fido-authenticator hashes for the
    /// pinUvAuthParam (it deserializes the wire bytes into the struct and
    /// re-serializes with `cbor_smol::cbor_serialize_to`).
    fn cbor_smol_bytes(&self) -> Vec<u8> {
        // Borrowed deserialization needs `'static` backing bytes (rp_ids are
        // `&'a str`), so leak the intermediate CBOR like the other helpers do.
        let intermediate = serde_cbor::to_vec(&self.to_value()).expect("serialize params value");
        let leaked: &'static [u8] = Vec::leak(intermediate);
        let params: SubcommandParameters<'static> =
            serde_cbor::from_slice(leaked).expect("deserialize subcommand params");
        let mut buf = [0u8; ctap_types::ctap2::config::MAX_SUBCOMMAND_PARAMS_CBOR_LEN];
        let encoded =
            cbor_smol::cbor_serialize(&params, &mut buf).expect("serialize subcommand params");
        encoded.to_vec()
    }
}

/// Fetch the current GetInfo response.
fn get_info(authn: &mut dyn TestAuthenticator) -> ctap2::get_info::Response {
    match authn
        .call_ctap2(&Request::GetInfo)
        .expect("GetInfo should succeed")
    {
        Response::GetInfo(info) => info,
        other => panic!("Expected GetInfo, got {:?}", other),
    }
}

/// Acquire a PinUvAuthToken with the `acfg` permission.
fn acfg_token(authn: &mut dyn TestAuthenticator) -> PinSession {
    PinSession::try_get_pin_token_with_permissions(
        authn,
        PIN,
        Permissions::AUTHENTICATOR_CONFIGURATION,
    )
    .expect("GetPinUvAuthTokenUsingPinWithPermissions(acfg) should succeed")
}

/// Build the pinUvAuthParam over `0xff*32 || 0x0d || subCommand || cbor(params)`,
/// using the supplied PinUvAuthToken (HMAC-SHA-256, left 16 bytes).
fn config_pin_auth(pin: &PinSession, sub_command: u8, params: Option<&ParamsSpec>) -> Vec<u8> {
    let mut data: Vec<u8> = vec![0xff; 32];
    data.push(0x0d);
    data.push(sub_command);
    if let Some(params) = params {
        data.extend_from_slice(&params.cbor_smol_bytes());
    }
    pin.pin_auth_for_client_data_hash(&data).to_vec()
}

/// Deserialize a `Config` request from a CBOR `Value` (the request type is
/// `#[non_exhaustive]`, so we cannot build it with a struct literal).
fn config_request_from_value(value: Value) -> ctap2::config::Request<'static> {
    let encoded = serde_cbor::to_vec(&value).expect("serialize config request");
    let leaked: &'static [u8] = Vec::leak(encoded);
    serde_cbor::from_slice(leaked).expect("deserialize config request")
}

/// Build a `Config` request with the given subcommand, optional params, and a
/// valid `acfg` pinUvAuthParam.
fn config_request(
    pin: &PinSession,
    sub_command: u8,
    params: Option<ParamsSpec>,
) -> ctap2::config::Request<'static> {
    let pin_auth = config_pin_auth(pin, sub_command, params.as_ref());
    let mut entries = vec![(Value::Integer(1), Value::Integer(sub_command as i128))];
    if let Some(params) = &params {
        entries.push((Value::Integer(2), params.to_value()));
    }
    entries.push((Value::Integer(3), Value::Integer(pin.protocol() as i128)));
    entries.push((Value::Integer(4), Value::Bytes(pin_auth)));
    config_request_from_value(Value::Map(entries.into_iter().collect()))
}

/// Build a raw `Config` request `Value` with caller-controlled pin fields, used
/// for the negative cases (missing / bogus pinUvAuthParam).
fn config_request_raw(
    sub_command: u8,
    pin_protocol: Option<u8>,
    pin_auth: Option<Vec<u8>>,
) -> ctap2::config::Request<'static> {
    let mut entries = vec![(Value::Integer(1), Value::Integer(sub_command as i128))];
    if let Some(p) = pin_protocol {
        entries.push((Value::Integer(3), Value::Integer(p as i128)));
    }
    if let Some(a) = pin_auth {
        entries.push((Value::Integer(4), Value::Bytes(a)));
    }
    config_request_from_value(Value::Map(entries.into_iter().collect()))
}

/// P-2: toggleAlwaysUv(0x02) flips GetInfo.options.alwaysUv.
///
/// The device supports disabling alwaysUv, so the success branch applies:
/// alwaysUv starts false, becomes true after one toggle, and false again
/// after a second toggle.
#[test]
#[serial]
fn config_toggle_always_uv() {
    run_isolated_in_sim("authenticator_config::config_toggle_always_uv", || {
        run_in_thread(|| {
            with_authenticator!(config_toggle_always_uv, |authn| {
                reset_authenticator(authn);
                PinSession::set_pin(authn, PIN);

                // authnrCfg must be advertised, and alwaysUv defaults to false.
                let options = get_info(authn).options.expect("options present");
                assert_eq!(
                    options.authnr_cfg,
                    Some(true),
                    "authnrCfg must be advertised"
                );
                let before = options.always_uv.expect("alwaysUv option present");
                assert!(!before, "alwaysUv should default to false");

                // First toggle: false -> true.
                let pin = acfg_token(authn);
                authn
                    .call_ctap2(&Request::Config(config_request(
                        &pin,
                        SUB_TOGGLE_ALWAYS_UV,
                        None,
                    )))
                    .expect("toggleAlwaysUv should succeed");
                let after = get_info(authn).options.unwrap().always_uv;
                assert_eq!(
                    after,
                    Some(!before),
                    "alwaysUv must be the opposite value after toggleAlwaysUv"
                );

                // Second toggle: true -> false. (Needs a fresh token; a PIN
                // token with acfg still authorizes config while alwaysUv is on.)
                let pin = acfg_token(authn);
                authn
                    .call_ctap2(&Request::Config(config_request(
                        &pin,
                        SUB_TOGGLE_ALWAYS_UV,
                        None,
                    )))
                    .expect("second toggleAlwaysUv should succeed");
                let restored = get_info(authn).options.unwrap().always_uv;
                assert_eq!(
                    restored,
                    Some(before),
                    "alwaysUv must toggle back to original"
                );
            })
        });
    });
}

/// P-3: setMinPINLength(0x03) with newMinPINLength larger than the current
/// PINCodePointLength succeeds, and (since clientPin is set) forces a PIN change.
#[test]
#[serial]
fn config_set_min_pin_length() {
    run_isolated_in_sim("authenticator_config::config_set_min_pin_length", || {
        run_in_thread(|| {
            with_authenticator!(config_set_min_pin_length, |authn| {
                reset_authenticator(authn);
                // 6-character PIN: PINCodePointLength = 6.
                PinSession::set_pin(authn, PIN);

                let info = get_info(authn);
                assert_eq!(
                    info.min_pin_length,
                    Some(4),
                    "default minPINLength should be 4"
                );
                assert_eq!(
                    info.options.unwrap().set_min_pin_length,
                    Some(true),
                    "setMinPINLength must be advertised"
                );

                // New min PIN length (8) > current PINCodePointLength (6),
                // so this MUST set forcePINChange.
                let new_min = 8u8;
                let params = ParamsSpec {
                    new_min_pin_length: Some(new_min),
                    ..Default::default()
                };

                let pin = acfg_token(authn);
                authn
                    .call_ctap2(&Request::Config(config_request(
                        &pin,
                        SUB_SET_MIN_PIN_LENGTH,
                        Some(params),
                    )))
                    .expect("setMinPINLength should succeed");

                let info = get_info(authn);
                assert_eq!(
                    info.min_pin_length,
                    Some(new_min),
                    "minPINLength must reflect the new value"
                );
                assert_eq!(
                    info.force_pin_change,
                    Some(true),
                    "forcePINChange must be true when new minPINLength > PINCodePointLength and clientPin is set"
                );
            })
        });
    });
}

/// setMinPINLength with a value below the current minimum is rejected with
/// CTAP2_ERR_PIN_POLICY_VIOLATION (CTAP 2.1 §6.11.4 step 2.3).
#[test]
#[serial]
fn config_set_min_pin_length_cannot_lower() {
    run_isolated_in_sim(
        "authenticator_config::config_set_min_pin_length_cannot_lower",
        || {
            run_in_thread(|| {
                with_authenticator!(config_set_min_pin_length_cannot_lower, |authn| {
                    reset_authenticator(authn);
                    PinSession::set_pin(authn, PIN);

                    // Raise the floor to 8 first. Reuse this same acfg token for
                    // the lowering attempt below: raising minPINLength sets
                    // forcePINChange=true (clientPin is set), and once
                    // forcePINChange is pending the authenticator refuses to
                    // ISSUE a new pinUvAuthToken (PinPolicyViolation). An
                    // already-issued token is still accepted for config, so we
                    // keep `pin` rather than re-acquiring it.
                    let pin = acfg_token(authn);
                    authn
                        .call_ctap2(&Request::Config(config_request(
                            &pin,
                            SUB_SET_MIN_PIN_LENGTH,
                            Some(ParamsSpec {
                                new_min_pin_length: Some(8),
                                ..Default::default()
                            }),
                        )))
                        .expect("raising minPINLength should succeed");

                    // Attempt to lower below the new floor -> PinPolicyViolation.
                    let result = authn.call_ctap2(&Request::Config(config_request(
                        &pin,
                        SUB_SET_MIN_PIN_LENGTH,
                        Some(ParamsSpec {
                            new_min_pin_length: Some(6),
                            ..Default::default()
                        }),
                    )));
                    assert_eq!(result, Err(ctap2::Error::PinPolicyViolation));
                })
            });
        },
    );
}

/// P-4: setMinPINLength(0x03) with minPinLengthRPIDs (up to
/// maxRPIDsForSetMinPINLength = 4 RP IDs) succeeds, since the device supports
/// the minPinLength extension.
#[test]
#[serial]
fn config_set_min_pin_length_rp_ids() {
    run_isolated_in_sim(
        "authenticator_config::config_set_min_pin_length_rp_ids",
        || {
            run_in_thread(|| {
                with_authenticator!(config_set_min_pin_length_rp_ids, |authn| {
                    reset_authenticator(authn);
                    PinSession::set_pin(authn, PIN);

                    let info = get_info(authn);
                    let max_rpids = info
                        .max_rpids_for_set_min_pin_length
                        .expect("maxRPIDsForSetMinPINLength present");
                    assert!(max_rpids >= 1, "device should allow at least one RP ID");
                    // minPinLength extension must be advertised.
                    let extensions = info.extensions.expect("extensions present");
                    assert!(
                        extensions.contains(&ctap2::get_info::Extension::MinPinLength),
                        "minPinLength extension must be advertised"
                    );

                    // Fill the RP-ID list up to the advertised max.
                    let candidates = [
                        "rp0.example.com",
                        "rp1.example.com",
                        "rp2.example.com",
                        "rp3.example.com",
                    ];
                    let rp_ids: Vec<String> = candidates
                        .iter()
                        .take(max_rpids)
                        .map(|s| s.to_string())
                        .collect();

                    let params = ParamsSpec {
                        min_pin_length_rp_ids: Some(rp_ids),
                        ..Default::default()
                    };

                    let pin = acfg_token(authn);
                    authn
                        .call_ctap2(&Request::Config(config_request(
                            &pin,
                            SUB_SET_MIN_PIN_LENGTH,
                            Some(params),
                        )))
                        .expect("setMinPINLength with minPinLengthRPIDs should succeed");
                })
            });
        },
    );
}

/// P-5: setMinPINLength(0x03) with forceChangePin=true (and clientPin set)
/// succeeds and sets GetInfo.forcePINChange to true.
#[test]
#[serial]
fn config_set_min_pin_length_force_change_pin() {
    run_isolated_in_sim(
        "authenticator_config::config_set_min_pin_length_force_change_pin",
        || {
            run_in_thread(|| {
                with_authenticator!(config_set_min_pin_length_force_change_pin, |authn| {
                    reset_authenticator(authn);
                    PinSession::set_pin(authn, PIN);

                    // Right after set_pin, forcePINChange should be false.
                    assert_eq!(get_info(authn).force_pin_change, Some(false));

                    let params = ParamsSpec {
                        force_change_pin: Some(true),
                        ..Default::default()
                    };

                    let pin = acfg_token(authn);
                    authn
                        .call_ctap2(&Request::Config(config_request(
                            &pin,
                            SUB_SET_MIN_PIN_LENGTH,
                            Some(params),
                        )))
                        .expect("setMinPINLength with forceChangePin should succeed");

                    assert_eq!(
                        get_info(authn).force_pin_change,
                        Some(true),
                        "forcePINChange must be true after forceChangePin=true"
                    );
                })
            });
        },
    );
}

/// Negative: with a PIN set, an authenticatorConfig request that omits the
/// pinUvAuthParam is rejected with CTAP2_ERR_PUAT_REQUIRED (mapped to
/// PinRequired, CTAP 2.1 §6.11 step 4.1).
#[test]
#[serial]
fn config_requires_pin_uv_auth() {
    run_isolated_in_sim("authenticator_config::config_requires_pin_uv_auth", || {
        run_in_thread(|| {
            with_authenticator!(config_requires_pin_uv_auth, |authn| {
                reset_authenticator(authn);
                PinSession::set_pin(authn, PIN);

                let result = authn.call_ctap2(&Request::Config(config_request_raw(
                    SUB_TOGGLE_ALWAYS_UV,
                    None,
                    None,
                )));
                assert_eq!(result, Err(ctap2::Error::PinRequired));
            })
        });
    });
}

/// Negative: a pinUvAuthParam that does not authenticate the correct data is
/// rejected with CTAP2_ERR_PIN_AUTH_INVALID (CTAP 2.1 §6.11 step 4.4).
#[test]
#[serial]
fn config_invalid_pin_uv_auth_param() {
    run_isolated_in_sim(
        "authenticator_config::config_invalid_pin_uv_auth_param",
        || {
            run_in_thread(|| {
                with_authenticator!(config_invalid_pin_uv_auth_param, |authn| {
                    reset_authenticator(authn);
                    PinSession::set_pin(authn, PIN);
                    let pin = acfg_token(authn);

                    // A bogus 16-byte pinUvAuthParam that doesn't match the data.
                    let result = authn.call_ctap2(&Request::Config(config_request_raw(
                        SUB_TOGGLE_ALWAYS_UV,
                        Some(pin.protocol()),
                        Some(vec![0x00u8; 16]),
                    )));
                    assert_eq!(result, Err(ctap2::Error::PinAuthInvalid));
                })
            });
        },
    );
}

/// Negative: a PinUvAuthToken lacking the `acfg` permission is rejected with
/// CTAP2_ERR_PIN_AUTH_INVALID (CTAP 2.1 §6.11 step 4.5).
#[test]
#[serial]
fn config_requires_acfg_permission() {
    run_isolated_in_sim(
        "authenticator_config::config_requires_acfg_permission",
        || {
            run_in_thread(|| {
                with_authenticator!(config_requires_acfg_permission, |authn| {
                    reset_authenticator(authn);
                    PinSession::set_pin(authn, PIN);

                    // Token granted only MAKE_CREDENTIAL — no acfg permission.
                    let pin = PinSession::try_get_pin_token_with_permissions(
                        authn,
                        PIN,
                        Permissions::MAKE_CREDENTIAL,
                    )
                    .expect("get token with mc permission");

                    let result = authn.call_ctap2(&Request::Config(config_request(
                        &pin,
                        SUB_TOGGLE_ALWAYS_UV,
                        None,
                    )));
                    assert_eq!(result, Err(ctap2::Error::PinAuthInvalid));
                })
            });
        },
    );
}
