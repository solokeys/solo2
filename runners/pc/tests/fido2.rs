//! FIDO2 authenticator test suite.
//!
//! Runs against either an in-process PC runner or a real USB device.
//! Set `FIDO2_TRANSPORT=device` to target hardware.

use ctap_types::ctap2::{self, Request, Response};
#[allow(unused_imports)]
use fido_authenticator::{Authenticator, Config, Conforming, Silent};
use serde_cbor::value::to_value;
use serde_cbor::Value;

use serial_test::serial;

mod support;
use support::transport::{self, Backend, DeviceTransport, TestAuthenticator};
use support::up;

fn run_in_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    const ISOLATED_ENV: &str = "FIDO2_ISOLATED_TEST";

    if transport::backend() == Backend::Sim {
        if let Some(test_name) = std::thread::current().name() {
            if std::env::var(ISOLATED_ENV).ok().as_deref() != Some(test_name) {
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .arg("--exact")
                    .arg(test_name)
                    .arg("--test-threads=1")
                    .env(ISOLATED_ENV, test_name)
                    .status()
                    .expect("spawn isolated test subprocess");

                assert!(
                    status.success(),
                    "isolated test subprocess failed: {}",
                    test_name
                );
                return;
            }
        }
    }

    std::thread::Builder::new()
        .name("fido-test".into())
        // Host-side authenticator stack is deep (Trussed + crypto); 256 KiB overflows on macOS.
        .stack_size(8 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

fn run_isolated_in_sim<F>(test_name: &str, f: F)
where
    F: FnOnce() + Send + 'static,
{
    const ISOLATED_ENV: &str = "FIDO2_ISOLATED_TEST";

    if transport::backend() != Backend::Sim
        || std::env::var(ISOLATED_ENV).ok().as_deref() == Some(test_name)
    {
        f();
        return;
    }

    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(test_name)
        .arg("--test-threads=1")
        .env(ISOLATED_ENV, test_name)
        .status()
        .expect("spawn isolated test subprocess");

    assert!(
        status.success(),
        "isolated test subprocess failed: {}",
        test_name
    );
}

// =============================================================================
// The core abstraction: `authenticator!` returns a `Box<dyn TestAuthenticator>`
// regardless of backend. Tests never branch on transport mode.
// =============================================================================

/// Run test body against any authenticator backend.
///
/// - `FIDO2_TRANSPORT=device`: USB HID to real hardware
/// - `FIDO2_TRANSPORT=socket`: Unix socket to PC runner simulator
/// - unset/default: in-process simulator
///
/// The `$body` receives `&mut dyn TestAuthenticator`.
macro_rules! with_authenticator {
    ($name:ident, |$authn:ident| $body:block) => {
        with_authenticator!($name, Conforming {}, |$authn| $body)
    };
    ($name:ident, $up:expr, |$authn:ident| $body:block) => {
        match transport::backend() {
            Backend::Device => {
                let mut dev = DeviceTransport::open_hid();
                let $authn: &mut dyn TestAuthenticator = &mut dev;
                $body
            }
            Backend::Socket => {
                let mut sock = DeviceTransport::open_socket();
                let $authn: &mut dyn TestAuthenticator = &mut sock;
                $body
            }
            Backend::Sim => support::sim::with_client(|client| {
                let mut sim = Authenticator::new(
                    client,
                    $up,
                    Config {
                        max_msg_size: 7609,
                        skip_up_timeout: None,
                        max_resident_credential_count: None,
                        large_blobs: None,
                        nfc_transport: false,
                    },
                );
                let $authn: &mut dyn TestAuthenticator = &mut sim;
                $body
            }),
        }
    };
}

// --- Shared request builders ---
//
// Request types in ctap-types 0.5 carry a lifetime (e.g. `Request<'a>`) with
// borrowed `&'a serde_bytes::Bytes` fields. For short-lived test helpers we
// leak the backing buffers so the returned `Request<'static>` keeps the APIs
// ergonomic (the process exits when tests finish).

/// Leak `data` as `&'static serde_bytes::Bytes`.
fn leak_bytes(data: impl Into<Vec<u8>>) -> &'static serde_bytes::Bytes {
    let leaked: &'static [u8] = Vec::leak(data.into());
    serde_bytes::Bytes::new(leaked)
}

/// Leak `s` as `&'static str`.
fn leak_str(s: impl Into<String>) -> &'static str {
    String::leak(s.into())
}

fn decode_from_value<T>(value: Value) -> T
where
    T: serde::de::DeserializeOwned,
{
    let encoded = serde_cbor::to_vec(&value).expect("serialize request");
    serde_cbor::from_slice(&encoded).expect("deserialize request")
}

fn make_credential_request_from_value(value: Value) -> ctap2::make_credential::Request<'static> {
    let encoded = serde_cbor::to_vec(&value).expect("serialize makeCredential request");
    let leaked: &'static [u8] = Vec::leak(encoded);
    serde_cbor::from_slice(leaked).expect("deserialize makeCredential request")
}

fn get_assertion_request_from_value(value: Value) -> ctap2::get_assertion::Request<'static> {
    let encoded = serde_cbor::to_vec(&value).expect("serialize getAssertion request");
    let leaked: &'static [u8] = Vec::leak(encoded);
    serde_cbor::from_slice(leaked).expect("deserialize getAssertion request")
}

fn options_value(rk: Option<bool>, up: Option<bool>, uv: Option<bool>) -> Value {
    let mut entries = vec![];
    if let Some(rk) = rk {
        entries.push((Value::Text("rk".to_string()), Value::Bool(rk)));
    }
    if let Some(up) = up {
        entries.push((Value::Text("up".to_string()), Value::Bool(up)));
    }
    if let Some(uv) = uv {
        entries.push((Value::Text("uv".to_string()), Value::Bool(uv)));
    }
    Value::Map(entries.into_iter().collect())
}

fn make_credential_request() -> ctap2::make_credential::Request<'static> {
    make_credential_request_for("example.com", &[0x01; 16], "testuser", false)
}

fn make_credential_request_for(
    rp_id: &str,
    user_id: &[u8],
    user_name: &str,
    resident_key: bool,
) -> ctap2::make_credential::Request<'static> {
    use ctap_types::webauthn::*;

    let mut params = FilteredPublicKeyCredentialParameters(heapless::Vec::new());
    params
        .0
        .push(KnownPublicKeyCredentialParameters { alg: -7 })
        .ok();

    let rp = PublicKeyCredentialRpEntity {
        id: rp_id.try_into().unwrap(),
        name: Some("Example".try_into().unwrap()),
        icon: None,
    };
    let user = PublicKeyCredentialUserEntity {
        id: ctap_types::Bytes::try_from(user_id).unwrap(),
        icon: None,
        name: Some(user_name.try_into().unwrap()),
        display_name: Some("Test User".try_into().unwrap()),
    };

    let mut req: ctap2::make_credential::Request<'static> =
        make_credential_request_from_value(Value::Map(
            [
                (Value::Integer(1), Value::Bytes([0xcd_u8; 32].to_vec())),
                (Value::Integer(2), to_value(&rp).expect("serialize rp")),
                (Value::Integer(3), to_value(&user).expect("serialize user")),
                (
                    Value::Integer(4),
                    to_value(&params).expect("serialize pub key cred params"),
                ),
            ]
            .into_iter()
            .collect(),
        ));
    if resident_key {
        req.options = Some(decode_from_value(options_value(Some(true), None, None)));
    }
    req
}

fn get_assertion_request(credential_id: &[u8]) -> ctap2::get_assertion::Request<'static> {
    get_assertion_request_for("example.com", Some(single_allow_list(credential_id)))
}

fn get_assertion_request_for(
    rp_id: &str,
    allow_list: Option<ctap2::get_assertion::AllowList<'static>>,
) -> ctap2::get_assertion::Request<'static> {
    let mut req: ctap2::get_assertion::Request<'static> =
        get_assertion_request_from_value(Value::Map(
            [
                (Value::Integer(1), Value::Text(rp_id.to_string())),
                (Value::Integer(2), Value::Bytes([0xcd_u8; 32].to_vec())),
            ]
            .into_iter()
            .collect(),
        ));
    req.allow_list = allow_list;
    req
}

fn single_allow_list(credential_id: &[u8]) -> ctap2::get_assertion::AllowList<'static> {
    let mut allow_list: ctap2::get_assertion::AllowList<'static> = ctap_types::Vec::new();
    allow_list.push(descriptor_ref(credential_id)).ok().unwrap();
    allow_list
}

/// Build a `PublicKeyCredentialDescriptorRef<'static>` for `credential_id`, leaking the bytes.
fn descriptor_ref(
    credential_id: &[u8],
) -> ctap_types::webauthn::PublicKeyCredentialDescriptorRef<'static> {
    ctap_types::webauthn::PublicKeyCredentialDescriptorRef {
        id: leak_bytes(credential_id.to_vec()),
        key_type: "public-key",
    }
}

/// Build a `PublicKeyCredentialDescriptorRef<'static>` with an arbitrary key_type string.
fn descriptor_ref_typed(
    credential_id: &[u8],
    key_type: &str,
) -> ctap_types::webauthn::PublicKeyCredentialDescriptorRef<'static> {
    ctap_types::webauthn::PublicKeyCredentialDescriptorRef {
        id: leak_bytes(credential_id.to_vec()),
        key_type: leak_str(key_type.to_string()),
    }
}

/// Build a `FilteredPublicKeyCredentialParameters` from the given algorithm list.
fn pkcp_for(algs: &[i32]) -> ctap_types::webauthn::FilteredPublicKeyCredentialParameters {
    use ctap_types::webauthn::{
        FilteredPublicKeyCredentialParameters, KnownPublicKeyCredentialParameters,
    };
    let mut inner = heapless::Vec::new();
    for alg in algs {
        let _ = inner.push(KnownPublicKeyCredentialParameters { alg: *alg });
    }
    FilteredPublicKeyCredentialParameters(inner)
}

fn extract_credential_id(auth_data: &[u8]) -> Vec<u8> {
    let offset = 32 + 1 + 4 + 16;
    let len = u16::from_be_bytes([auth_data[offset], auth_data[offset + 1]]) as usize;
    auth_data[offset + 2..offset + 2 + len].to_vec()
}

fn make_credential(authn: &mut dyn TestAuthenticator) -> Vec<u8> {
    let resp = authn
        .call_ctap2(&Request::MakeCredential(make_credential_request()))
        .expect("MakeCredential failed");
    match resp {
        Response::MakeCredential(mc) => extract_credential_id(&mc.auth_data),
        other => panic!("Expected MakeCredential, got {:?}", other),
    }
}

// --- Device reset helper ---

/// Reboot the device (device mode only) so CTAP2 Reset is within the 10s window.
fn device_reboot() {
    if !transport::is_device_mode() {
        return;
    }
    let chip = std::env::var("PROBE_RS_CHIP").unwrap_or("LPC55S69JBD100".into());
    let protocol = std::env::var("PROBE_RS_PROTOCOL").ok();
    let speed = std::env::var("PROBE_RS_SPEED").ok();
    let mut cmd = std::process::Command::new("probe-rs");
    cmd.args(["reset", "--chip", &chip]);
    if let Some(p) = protocol.as_deref() {
        cmd.args(["--protocol", p]);
    }
    if let Some(s) = speed.as_deref() {
        cmd.args(["--speed", s]);
    }
    let _ = cmd.status();
    std::thread::sleep(std::time::Duration::from_secs(1));
}

/// Reset the authenticator to a clean state (no credentials, no PIN).
/// Reboots the device (device mode), reconnects, then sends CTAP2 Reset.
fn reset_authenticator(authn: &mut dyn TestAuthenticator) {
    device_reboot();
    authn.reconnect();
    up::approve();
    let _ = authn.call_ctap2(&Request::Reset);
}

// --- Submodules ---

#[path = "fido2/get_info.rs"]
mod get_info;

#[path = "fido2/make_credential.rs"]
mod make_credential;

#[path = "fido2/get_assertion.rs"]
mod get_assertion;

#[path = "fido2/get_assertion_parity.rs"]
mod get_assertion_parity;

#[path = "fido2/resident_key.rs"]
mod resident_key;

#[path = "fido2/credential_management.rs"]
mod credential_management;

#[path = "fido2/pin.rs"]
mod pin;

#[path = "fido2/reset.rs"]
mod reset;

#[path = "fido2/user_presence.rs"]
mod user_presence;

#[path = "fido2/cred_protect.rs"]
mod cred_protect;

#[path = "fido2/hmac_secret.rs"]
mod hmac_secret;
