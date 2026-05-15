//! authenticatorLargeBlobs (0x0C) command tests.
//!
//! Ports the FIDO CTAP2.3 conformance category
//! `tests/CTAP2/Protocol/LargeBlobs/LargeBlobs-1.js` (helper logic in
//! `js/LargeBlob2.1.js`).
//!
//! The serialized large-blob array is `<data> || SHA256(<data>)[..16]`. For
//! the empty array the data is the single CBOR byte `0x80` (empty CBOR array),
//! so the initial stored value is `0x80 || SHA256(0x80)[..16]` (17 bytes).
//!
//! DEVICE-ONLY: the sim `with_authenticator!` config hardcodes
//! `large_blobs: None`, so the command is unsupported in sim. Every test fn is
//! gated with `if !transport::is_device_mode() { return; }` so they compile and
//! trivially pass in sim/CI, and actually exercise on a real device.

use super::*;
use sha2::{Digest, Sha256};
use support::pin::PinSession;

/// SHA-256 of `data`, truncated to the first 16 bytes (the trailing checksum of
/// a serialized large-blob array).
fn truncated_hash(data: &[u8]) -> Vec<u8> {
    Sha256::digest(data)[..16].to_vec()
}

/// The serialized large-blob array carrying `data`: `data || SHA256(data)[..16]`.
fn serialized_large_blob_array(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    out.extend_from_slice(&truncated_hash(data));
    out
}

/// The initial (empty) serialized large-blob array: `0x80 || SHA256(0x80)[..16]`.
fn empty_large_blob_array() -> Vec<u8> {
    serialized_large_blob_array(&[0x80])
}

/// Build a `large_blobs::Request<'static>` from CBOR map entries (keyed by the
/// CTAP integer field ids). `large_blobs::Request` is `#[non_exhaustive]`, so it
/// cannot be struct-literal constructed from this crate; we go through CBOR
/// (mirroring `make_credential_request_from_value`/`client_pin_request_*`), and
/// leak the backing buffer so the borrowed fields are `'static`.
fn large_blobs_request_from_value(value: Value) -> ctap2::large_blobs::Request<'static> {
    let encoded = serde_cbor::to_vec(&value).expect("serialize LargeBlobs request");
    let leaked: &'static [u8] = Vec::leak(encoded);
    serde_cbor::from_slice(leaked).expect("deserialize LargeBlobs request")
}

/// Issue a LargeBlobs `get(offset, length)` and return the `config` bytes.
fn large_blobs_get(authn: &mut dyn TestAuthenticator, offset: u32, length: u32) -> Vec<u8> {
    // 0x01 get, 0x03 offset
    let req = large_blobs_request_from_value(Value::Map(
        [
            (Value::Integer(1), Value::Integer(length as i128)),
            (Value::Integer(3), Value::Integer(offset as i128)),
        ]
        .into_iter()
        .collect(),
    ));
    match authn
        .call_ctap2(&Request::LargeBlobs(req))
        .expect("LargeBlobs get should succeed")
    {
        Response::LargeBlobs(resp) => resp
            .config
            .expect("LargeBlobs get response missing config(0x01)")
            .as_slice()
            .to_vec(),
        other => panic!("Expected LargeBlobs response, got {:?}", other),
    }
}

/// pinUvAuthParam for a LargeBlobs `set`: HMAC-SHA256-left-16 over
/// `0xff*32 || 0x0c 0x00 || offset_LE(4) || SHA256(set_bytes)`.
fn large_blobs_set_pin_auth(pin: &PinSession, offset: u32, set_bytes: &[u8]) -> [u8; 16] {
    let mut message = Vec::new();
    message.extend_from_slice(&[0xff; 32]);
    message.extend_from_slice(&[0x0c, 0x00]);
    message.extend_from_slice(&offset.to_le_bytes());
    message.extend_from_slice(&Sha256::digest(set_bytes));
    // `pin_auth_for_client_data_hash` is just HMAC-left-16(token, message).
    pin.pin_auth_for_client_data_hash(&message)
}

/// Issue a LargeBlobs `set` writing the serialized array of `data` at `offset`.
/// On `offset == 0` the `length` field carries the total serialized length.
fn large_blobs_set(
    authn: &mut dyn TestAuthenticator,
    pin: &PinSession,
    offset: u32,
    data: &[u8],
) -> Result<(), ctap2::Error> {
    let set_bytes = serialized_large_blob_array(data);
    let pin_auth = large_blobs_set_pin_auth(pin, offset, &set_bytes);

    // 0x02 set, 0x03 offset, 0x04 length (offset==0 only), 0x05 pinUvAuthParam,
    // 0x06 pinUvAuthProtocol.
    let mut entries = vec![
        (Value::Integer(2), Value::Bytes(set_bytes.clone())),
        (Value::Integer(3), Value::Integer(offset as i128)),
    ];
    if offset == 0 {
        entries.push((Value::Integer(4), Value::Integer(set_bytes.len() as i128)));
    }
    entries.push((Value::Integer(5), Value::Bytes(pin_auth.to_vec())));
    entries.push((Value::Integer(6), Value::Integer(pin.protocol() as i128)));

    let req = large_blobs_request_from_value(Value::Map(entries.into_iter().collect()));
    authn.call_ctap2(&Request::LargeBlobs(req)).map(|_| ())
}

/// P-1: if `maxSerializedLargeBlobArray` is present in GetInfo, it must be at
/// least 1024.
#[test]
#[serial]
fn large_blobs_max_serialized_at_least_1024() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_max_serialized, |authn| {
            reset_authenticator(authn);
            let info = match authn.call_ctap2(&Request::GetInfo).expect("GetInfo failed") {
                Response::GetInfo(info) => info,
                other => panic!("Expected GetInfo, got {:?}", other),
            };
            if let Some(max) = info.max_serialized_large_blob_array {
                assert!(
                    max >= 1024,
                    "maxSerializedLargeBlobArray must be at least 1024, got {max}"
                );
            }
        })
    });
}

/// P-2: get with length 0 at offset 0 returns an empty byte string.
#[test]
#[serial]
fn large_blobs_get_zero_length_returns_empty() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_get_zero, |authn| {
            reset_authenticator(authn);
            let config = large_blobs_get(authn, 0, 0);
            assert_eq!(
                config.len(),
                0,
                "authenticator returned bytes when none was requested"
            );
        })
    });
}

/// P-3: get with length > 17 at offset 0 returns the 17-byte initial array
/// `0x80 || SHA256(0x80)[..16]`.
#[test]
#[serial]
fn large_blobs_get_initial_empty_array() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_get_initial, |authn| {
            reset_authenticator(authn);
            // Length 100 > 17, so we expect the full 17-byte initial array.
            let config = large_blobs_get(authn, 0, 100);
            let expected = empty_large_blob_array();
            assert_eq!(
                config.len(),
                17,
                "expected the 17-byte initial serialized large-blob array"
            );
            assert_eq!(
                config, expected,
                "expected 0x80 || first 16 bytes of SHA256(0x80)"
            );
        })
    });
}

/// P-4: set a random buffer at offset 0, then get it back whole and as a slice.
#[test]
#[serial]
fn large_blobs_set_then_get() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(lb_set_get, |authn| {
            reset_authenticator(authn);

            const PIN: &str = "123456A";
            PinSession::set_pin(authn, PIN);
            let pin = PinSession::try_get_pin_token_with_permissions(
                authn,
                PIN,
                ctap2::client_pin::Permissions::LARGE_BLOB_WRITE,
            )
            .expect("GetPinUvAuthToken with LARGE_BLOB_WRITE should succeed");

            // Deterministic "random" data (32 bytes) — the conformance test uses
            // a random length 20..100, but the framework asserts exact equality,
            // so any fixed buffer exercises the same path.
            let data: Vec<u8> = (0..32u8)
                .map(|i| i.wrapping_mul(7).wrapping_add(3))
                .collect();
            let expected = serialized_large_blob_array(&data);

            up::approve();
            large_blobs_set(authn, &pin, 0, &data).expect("LargeBlobs set should succeed");

            // Get the whole thing back.
            let config = large_blobs_get(authn, 0, 200);
            assert_eq!(
                config.len(),
                data.len() + 16,
                "expected set data + first 16 bytes of SHA256"
            );
            assert_eq!(
                config, expected,
                "expected the set data + first 16 bytes of its SHA256"
            );

            // Get a sub-slice (offset, length) and check it matches.
            let offset = 5usize;
            let length = 10usize;
            let slice = large_blobs_get(authn, offset as u32, length as u32);
            assert_eq!(
                slice,
                &expected[offset..offset + length],
                "expected the parameter-specified slice of the serialized array"
            );
        })
    });
}
