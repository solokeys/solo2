//! minPinLength extension tests (CTAP 2.1 §10.1.2.1 / §6.11.4).
//!
//! Ported from the FIDO conformance module
//! `tests/CTAP2/Protocol/Extensions/minPINLength.js`.
//!
//! The extension lets an RP request the authenticator's current minimum PIN
//! length on MakeCredential. The value is only emitted for RP IDs the platform
//! has allowlisted via `authenticatorConfig.setMinPINLength` (sub-command 0x03,
//! `minPinLengthRPIDs` 0x02). For any other RP the authenticator MUST ignore
//! the extension and emit no extension output.
//!
//! On a factory-reset authenticator (no PIN, alwaysUv off) `authenticatorConfig`
//! is invokable without a pinUvAuthParam (CTAP 2.1 §6.11 step 4 note), so these
//! tests configure the allowed-RP list directly and never need a PIN.

use super::*;

const MIN_PIN_LENGTH_FLOOR: u64 = 4;

/// `setMinPINLength` sub-command (CTAP 2.1 §6.11).
const SUBCMD_SET_MIN_PIN_LENGTH: i128 = 0x03;

/// AT (attested credential data) flag in the authenticator-data flags byte.
const FLAG_AT: u8 = 0x40;
/// ED (extension data) flag in the authenticator-data flags byte.
const FLAG_ED: u8 = 0x80;

/// Allowlist `rp_id` for the minPinLength extension via
/// `authenticatorConfig.setMinPINLength` (no PIN required on a fresh device).
fn configure_allowed_rp(authn: &mut dyn TestAuthenticator, rp_id: &str) {
    // `authenticatorConfig` (0x0D) Request:
    //   0x01 sub_command = setMinPINLength (0x03)
    //   0x02 sub_command_params { 0x02 min_pin_length_rp_ids: [rp_id] }
    // The struct is `#[non_exhaustive]`, so build it from a CBOR Value and
    // deserialize (matching authenticator_config.rs's `config_request_from_value`).
    let params = Value::Map(
        [(
            Value::Integer(2),
            Value::Array(vec![Value::Text(rp_id.to_string())]),
        )]
        .into_iter()
        .collect(),
    );
    let value = Value::Map(
        [
            (Value::Integer(1), Value::Integer(SUBCMD_SET_MIN_PIN_LENGTH)),
            (Value::Integer(2), params),
        ]
        .into_iter()
        .collect(),
    );
    let encoded = serde_cbor::to_vec(&value).expect("serialize config request");
    let leaked: &'static [u8] = Vec::leak(encoded);
    let request: ctap2::config::Request<'static> =
        serde_cbor::from_slice(leaked).expect("deserialize config request");

    authn
        .call_ctap2(&Request::Config(request))
        .expect("setMinPINLength(minPinLengthRPIDs) should succeed on a fresh device");
}

/// Build a MakeCredential request for `rp_id` carrying `minPinLength: true`.
fn mc_with_min_pin_length(rp_id: &str) -> ctap2::make_credential::Request<'static> {
    let mut req = make_credential_request_for(rp_id, &[0x5a; 16], "min-pin-user", true);
    let mut ext = ctap2::make_credential::ExtensionsInput::default();
    ext.min_pin_length = Some(true);
    req.extensions = Some(ext);
    req
}

/// Extract the CBOR-encoded extensions map that trails the attested credential
/// data in `auth_data`, returning `None` when the ED flag is clear.
///
/// Layout (CTAP 2.1 §6.1): rpIdHash(32) | flags(1) | signCount(4) |
/// [attestedCredData] | [extensions]. With AT set the attested credential data
/// is aaguid(16) | credIdLen(2) | credId(L) | credentialPublicKey (one CBOR
/// item). The extensions map is the single CBOR item that follows.
fn auth_data_extensions(auth_data: &[u8]) -> Option<serde_cbor::Value> {
    let flags = auth_data[32];
    if flags & FLAG_ED == 0 {
        return None;
    }
    assert!(
        flags & FLAG_AT != 0,
        "MakeCredential auth_data must carry attested credential data",
    );

    // rpIdHash(32) + flags(1) + signCount(4) + aaguid(16) = 53
    let cred_id_len_off = 53;
    let cred_id_len =
        u16::from_be_bytes([auth_data[cred_id_len_off], auth_data[cred_id_len_off + 1]]) as usize;
    let pubkey_off = cred_id_len_off + 2 + cred_id_len;

    // The credential public key and the extensions map are two consecutive
    // CBOR items. Stream them so we can skip past the (variable-length) key.
    let mut de = serde_cbor::Deserializer::from_slice(&auth_data[pubkey_off..]);
    let _pubkey: serde_cbor::Value =
        serde::Deserialize::deserialize(&mut de).expect("decode credentialPublicKey");
    let extensions: serde_cbor::Value =
        serde::Deserialize::deserialize(&mut de).expect("decode extensions map");
    Some(extensions)
}

/// Look up the integer value of the `minPinLength` key in an extensions map.
fn min_pin_length_in(extensions: &serde_cbor::Value) -> Option<u64> {
    let serde_cbor::Value::Map(map) = extensions else {
        return None;
    };
    let value = map.get(&serde_cbor::Value::Text("minPinLength".to_string()))?;
    match value {
        serde_cbor::Value::Integer(n) => Some(*n as u64),
        _ => panic!("minPinLength extension output must be a NUMBER, got {value:?}"),
    }
}

/// P-1: a credential created with `minPinLength: true` for an allowlisted RP
/// returns the current minimum PIN length (>= 4) in the authData extensions.
#[test]
#[serial]
fn min_pin_length_returned_for_allowed_rp() {
    run_isolated_in_sim(
        "ext_min_pin_length::min_pin_length_returned_for_allowed_rp",
        || {
            run_in_thread(|| {
                with_authenticator!(min_pin_length_allowed, |authn| {
                    reset_authenticator(authn);

                    let rp_id = "min-pin-allowed.example.com";
                    configure_allowed_rp(authn, rp_id);

                    up::approve();
                    let mc = match authn
                        .call_ctap2(&Request::MakeCredential(mc_with_min_pin_length(rp_id)))
                        .expect("MC with minPinLength should succeed")
                    {
                        Response::MakeCredential(mc) => mc,
                        other => panic!("Expected MakeCredential, got {:?}", other),
                    };

                    let extensions = auth_data_extensions(&mc.auth_data).expect(
                        "authenticator must return extension data for an allowlisted minPinLength RP",
                    );
                    let value = min_pin_length_in(&extensions)
                        .expect("extensions map must contain the minPinLength extension output");
                    assert!(
                        value >= MIN_PIN_LENGTH_FLOOR,
                        "minPinLength must be at least 4, got {value}",
                    );
                })
            });
        },
    );
}

/// F-1: a credential created with `minPinLength: true` for an RP that is NOT on
/// the allowlist must ignore the extension and emit no extension output.
#[test]
#[serial]
fn min_pin_length_ignored_for_unlisted_rp() {
    run_isolated_in_sim(
        "ext_min_pin_length::min_pin_length_ignored_for_unlisted_rp",
        || {
            run_in_thread(|| {
                with_authenticator!(min_pin_length_unlisted, |authn| {
                    reset_authenticator(authn);

                    // Allowlist a DIFFERENT RP so the list is non-empty, then
                    // create a credential for an unlisted RP.
                    configure_allowed_rp(authn, "min-pin-other.example.com");

                    let rp_id = "min-pin-unlisted.example.com";
                    up::approve();
                    let mc = match authn
                        .call_ctap2(&Request::MakeCredential(mc_with_min_pin_length(rp_id)))
                        .expect("MC with minPinLength for an unlisted RP should still succeed")
                    {
                        Response::MakeCredential(mc) => mc,
                        other => panic!("Expected MakeCredential, got {:?}", other),
                    };

                    // No other extensions were requested, so the authenticator
                    // must leave the ED flag clear and emit no extension output.
                    assert!(
                        auth_data_extensions(&mc.auth_data)
                            .as_ref()
                            .and_then(min_pin_length_in)
                            .is_none(),
                        "minPinLength must not be returned for an RP outside the allowlist",
                    );
                })
            });
        },
    );
}
