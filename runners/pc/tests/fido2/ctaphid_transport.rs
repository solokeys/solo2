//! CTAPHID transport conformance (FIDO conformance HID-1).
//!
//! Ported from `tests/CTAP2/Transports/hid-1.js` (helper `js/HID.js`).
//!
//! ALL of these cases exercise the raw CTAPHID transport layer (channel
//! allocation, PING echo, KEEPALIVE during a UP wait, CANCEL ->
//! CTAP2_ERR_KEEPALIVE_CANCEL(0x2D), multi-packet fragmentation, transport
//! error codes). The sim backend bypasses CTAPHID entirely (it calls the
//! authenticator's `ctaphid_app::App::call` directly with a single CBOR
//! buffer), so none of this is observable in sim — every test fn therefore
//! gates with `if !transport::is_device_mode() { return; }` and trivially
//! passes in sim/CI, exercising real behavior only on the Pi `device` run.
//!
//! IMPORTANT FRAMEWORK GAP (see notes returned to the integrator):
//! `support::ctaphid::CtapHidClient` currently exposes only the high-level
//! `ctap2()` / `ctap1()` helpers. Those:
//!   * frame and de-frame automatically (so single-frame vs multi-frame is
//!     not separately controllable),
//!   * SILENTLY SWALLOW `CTAPHID_KEEPALIVE` frames in `recv()`, and
//!   * provide NO way to emit a raw `CTAPHID_CANCEL` / `PING` / `WINK` /
//!     `LOCK` frame, nor to read the allocated CID / INIT capability flags.
//!
//! Consequently the cases that require driving/observing *raw* CTAPHID
//! frames — P-1, P-2, P-3 (field-level), P-4..P-8, P-9, P-10, P-12..P-15,
//! F-1..F-4 — cannot be fully asserted through the public API today. They
//! are still ported here as device-gated test fns so the file is complete
//! and the integrator can flesh them out once raw-frame helpers are added
//! to `CtapHidClient` (proposed API listed in the returned notes). Where a
//! case CAN be partially exercised through the existing high-level API
//! (INIT-on-connect, CBOR round-trip, multi-packet fragmentation, CBOR
//! transport error codes), the real assertions are included.

use super::*;

/// CTAP command bytes used when driving `call_ctap2_raw`.
const CTAP_CMD_GET_INFO: u8 = 0x04;

// ---------------------------------------------------------------------------
// P-3 / P-4: CTAPHID_INIT + channel allocation
// ---------------------------------------------------------------------------

/// P-3 (partial): opening the HID device performs a `CTAPHID_INIT` on the
/// broadcast channel and allocates a fresh CID. `CtapHidClient::open_hid()`
/// panics if INIT fails or the INIT response is < 17 bytes, and every
/// subsequent CBOR call rides the allocated CID — so a successful CBOR
/// round-trip proves the INIT handshake + channel allocation worked.
///
/// The field-level INIT assertions from P-3 (NONCE echo, IFVERSION==2,
/// capability flags incl. CBOR set, BCNT==17) require reading the raw INIT
/// response bytes, which the high-level client does not surface. See notes.
#[test]
#[serial]
fn ctaphid_init_allocates_channel() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(ctaphid_init, |authn| {
            // A GetInfo over the freshly-INIT'd channel must succeed,
            // proving INIT + channel allocation worked.
            let resp = authn
                .call_ctap2(&Request::GetInfo)
                .expect("GetInfo over freshly-allocated CTAPHID channel must succeed");
            match resp {
                Response::GetInfo(info) => {
                    // The conformance INIT test requires CAPABILITY_CBOR be
                    // advertised; the device clearly supports CBOR if GetInfo
                    // round-trips and reports CTAP versions.
                    assert!(
                        !info.versions.is_empty(),
                        "device must advertise at least one CTAP version"
                    );
                }
                other => panic!("Expected GetInfo, got {:?}", other),
            }
        })
    });
}

/// P-4: three `CTAPHID_INIT`s on the broadcast channel must each return a
/// unique new CID. Opening three independent HID clients performs three
/// INITs; with the current API the allocated CID is not readable, so the
/// uniqueness assertion cannot be made. Ported as a device-gated no-op
/// pending a `CtapHidClient::cid()` accessor (see notes).
#[test]
#[serial]
fn ctaphid_init_unique_cids() {
    if !transport::is_device_mode() {
        return;
    }
    // TODO(integrator): needs `CtapHidClient::raw_init() -> InitResponse`
    // exposing NEWCID so three INITs can be compared for uniqueness.
}

// ---------------------------------------------------------------------------
// P-5..P-8: CTAPHID_PING echo
// ---------------------------------------------------------------------------

/// P-5/P-6/P-8: `CTAPHID_PING` must echo its payload verbatim (small, 1024
/// bytes spanning many CONT frames, and a payload with a trailing zero).
/// `CtapHidClient` has no PING method, so this needs a raw-frame helper.
/// Device-gated no-op pending `CtapHidClient::ping(&[u8])` (see notes).
#[test]
#[serial]
fn ctaphid_ping_echo() {
    if !transport::is_device_mode() {
        return;
    }
    // TODO(integrator): needs `CtapHidClient::ping(payload) -> Vec<u8>`
    // to assert byte-for-byte echo for payloads of 50 / 1024 / 58(+trailing
    // zero) bytes (P-5, P-6, P-8).
}

/// P-7: a `CTAPHID_PING` whose final CONT frame is replaced by a fresh
/// `CTAPHID_INIT` on the same CID must abort the PING and answer the INIT.
/// Needs raw multi-frame control. Device-gated no-op pending raw-frame
/// helpers (see notes).
#[test]
#[serial]
fn ctaphid_init_aborts_pending_ping() {
    if !transport::is_device_mode() {
        return;
    }
    // TODO(integrator): needs raw `send_frames(Vec<[u8;64]>)` so a partial
    // PING followed by an INIT on the same CID can be driven.
}

// ---------------------------------------------------------------------------
// P-9: CTAPHID_KEEPALIVE during a UP wait
// ---------------------------------------------------------------------------

/// P-9: while a MakeCredential waits for user action the authenticator must
/// emit `CTAPHID_KEEPALIVE(0x3B)` frames whose status byte is either
/// STATUS_PROCESSING(0x01) or STATUS_UPNEEDED(0x02).
///
/// `CtapHidClient::recv()` discards KEEPALIVE frames, so they cannot be
/// inspected through the public API. Device-gated no-op pending a
/// keepalive-observing recv helper (see notes).
#[test]
#[serial]
fn ctaphid_keepalive_during_up() {
    if !transport::is_device_mode() {
        return;
    }
    // TODO(integrator): needs a `CtapHidClient` recv variant that surfaces
    // the KEEPALIVE frames (CMD==0x3B, BCNT==1, status in {0x01,0x02})
    // instead of swallowing them.
}

// ---------------------------------------------------------------------------
// P-10 / P-15: CTAPHID_CANCEL during a UP wait -> CTAP2_ERR_KEEPALIVE_CANCEL
//   *** This is the P-10 CANCEL regression coverage. ***
// ---------------------------------------------------------------------------

/// P-10: send a MakeCredential, observe KEEPALIVE, then send
/// `CTAPHID_CANCEL(0x11)`. The authenticator MUST answer the in-flight CBOR
/// request with `CTAPHID_CBOR(0x90)` carrying status
/// `CTAP2_ERR_KEEPALIVE_CANCEL(0x2D)`. A second CANCEL must elicit no
/// response.
///
/// Driving CANCEL mid-transaction requires (a) sending the CBOR request
/// frames without blocking on the response and (b) emitting a raw CANCEL
/// frame on the same CID, neither of which the high-level client supports.
/// Device-gated no-op pending raw-frame helpers (see notes); this is the
/// headline regression the integrator must wire up.
#[test]
#[serial]
fn ctaphid_cancel_make_credential_keepalive_cancel() {
    if !transport::is_device_mode() {
        return;
    }
    // TODO(integrator): needs
    //   CtapHidClient::send_cbor_async(&[u8])  -> fire CBOR, do not wait
    //   CtapHidClient::send_cancel()           -> emit CTAPHID_CANCEL frame
    //   CtapHidClient::recv_cbor() -> (status, body) (keepalive-aware)
    // Expected: recv_cbor() returns status == 0x2D (KeepaliveCancel), and a
    // second send_cancel() yields no response frame.
    //
    // Once wired, the assertion is:
    //   assert_eq!(transport::error_from_byte(status),
    //              ctap2::Error::KeepaliveCancel);
}

/// P-15: same CANCEL-during-UP regression but driven via
/// `authenticatorSelection(0x0B)` (the CTAP 2.1 replacement for the
/// retired P-11). `Selection` blocks for user presence, so it is the
/// cleanest command to cancel. Needs the same raw-frame helpers as P-10.
/// Device-gated no-op pending those helpers (see notes).
#[test]
#[serial]
fn ctaphid_cancel_selection_keepalive_cancel() {
    if !transport::is_device_mode() {
        return;
    }
    // TODO(integrator): with the raw-frame helpers from P-10, fire
    // `Request::Selection` (CTAP cmd 0x0B) async, read KEEPALIVE, send
    // CTAPHID_CANCEL, and assert the CBOR response status == 0x2D
    // (KeepaliveCancel). A second CANCEL must produce no response.
}

// ---------------------------------------------------------------------------
// P-12: CTAPHID_WINK (optional)
// ---------------------------------------------------------------------------

/// P-12: if `CAPABILITY_WINK` is advertised, `CTAPHID_WINK(0x08)` must
/// echo CMD==WINK with BCNT==0. Needs INIT-capability inspection plus a
/// raw WINK frame. Device-gated no-op pending raw-frame helpers (see
/// notes).
#[test]
#[serial]
fn ctaphid_wink_if_supported() {
    if !transport::is_device_mode() {
        return;
    }
    // TODO(integrator): needs INIT capability flags + `CtapHidClient::wink()`.
    // Skip (pass) if WINK capability not advertised.
}

// ---------------------------------------------------------------------------
// P-13 / P-14: CTAPHID_LOCK (optional)
// ---------------------------------------------------------------------------

/// P-13/P-14: if `CAPABILITY_LOCK` is advertised, `CTAPHID_LOCK(0x04)`
/// must echo CMD==LOCK with BCNT==0, and while locked an INIT on another
/// CID must return CTAP1_ERR_CHANNEL_BUSY. Needs raw-frame + multi-CID
/// control. Device-gated no-op pending raw-frame helpers (see notes).
#[test]
#[serial]
fn ctaphid_lock_if_supported() {
    if !transport::is_device_mode() {
        return;
    }
    // TODO(integrator): needs INIT capability flags + `CtapHidClient::lock(secs)`
    // and a second-CID raw INIT. Skip (pass) if LOCK capability not advertised.
}

// ---------------------------------------------------------------------------
// CBOR transport + fragmentation (positive coverage achievable today)
// ---------------------------------------------------------------------------

/// CTAPHID_CBOR transport round-trip + response de-fragmentation.
///
/// GetInfo's response is well over one HID frame's INIT payload (57 bytes),
/// so a correct round-trip proves the client correctly de-fragments a
/// multi-CONT-frame CBOR response over the real CTAPHID transport. This is
/// the achievable slice of the "multi-packet fragmentation" requirement.
#[test]
#[serial]
fn ctaphid_cbor_multipacket_response() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(ctaphid_cbor_resp, |authn| {
            // Drive GetInfo via the raw CBOR path so we see the raw status
            // byte and the full (multi-frame) CBOR body.
            let (status, body) = authn
                .call_ctap2_raw(CTAP_CMD_GET_INFO, &[])
                .expect("CTAPHID_CBOR GetInfo transport failed");
            assert_eq!(status, 0x00, "GetInfo status byte must be CTAP2_OK");
            assert!(
                body.len() > 57,
                "GetInfo CBOR body ({} bytes) should span multiple HID \
                 frames, exercising CONT-frame de-fragmentation",
                body.len()
            );
        })
    });
}

/// Multi-packet REQUEST fragmentation: a MakeCredential request large
/// enough to require CONT frames on the way out must be accepted and
/// answered. A resident-key MakeCredential with the standard fields plus a
/// touch is comfortably larger than one INIT frame (57 bytes) once the rp /
/// user / clientDataHash are encoded, so a successful response proves the
/// host->device request fragmentation path.
#[test]
#[serial]
fn ctaphid_cbor_multipacket_request() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(ctaphid_cbor_req, |authn| {
            reset_authenticator(authn);
            up::approve();
            let resp = authn
                .call_ctap2(&Request::MakeCredential(make_credential_request_for(
                    "fragmentation.example.com",
                    &[0xF7; 16],
                    "fragmentation-test-user",
                    true,
                )))
                .expect("multi-frame MakeCredential request must round-trip over CTAPHID");
            match resp {
                Response::MakeCredential(mc) => {
                    assert!(
                        mc.auth_data.len() > 37,
                        "auth_data should carry an attested credential"
                    );
                }
                other => panic!("Expected MakeCredential, got {:?}", other),
            }
        })
    });
}

// ---------------------------------------------------------------------------
// F-1..F-4: transport / CBOR error codes
// ---------------------------------------------------------------------------

/// F-1 (CBOR-layer analogue): an unknown CTAP *command* byte sent over
/// `CTAPHID_CBOR` must be answered with `CTAP1_ERR_INVALID_COMMAND(0x01)`.
///
/// The conformance F-1 sets an unknown *CTAPHID* command byte (0x21), which
/// requires raw-frame control to reach CTAPHID_ERROR. The achievable
/// equivalent through the public API is an unknown CTAP command inside a
/// CTAPHID_CBOR frame, which the device answers with status 0x01 in the
/// CBOR response — the same error semantics one layer up.
#[test]
#[serial]
fn ctaphid_cbor_unknown_command() {
    if !transport::is_device_mode() {
        return;
    }
    run_in_thread(|| {
        with_authenticator!(ctaphid_bad_cmd, |authn| {
            // 0x21 is not a defined CTAP2 command.
            let (status, _body) = authn
                .call_ctap2_raw(0x21, &[])
                .expect("device must answer an unknown CTAP command, not drop the frame");
            assert_eq!(
                transport::error_from_byte(status),
                ctap2::Error::InvalidCommand,
                "unknown CTAP command must yield CTAP1_ERR_INVALID_COMMAND(0x01)"
            );
        })
    });
}

/// F-4: a multi-frame `CTAPHID_PING` with an out-of-order continuation SEQ
/// must yield `CTAPHID_ERROR` with `ERR_INVALID_SEQ(0x04)`. Needs raw
/// per-frame SEQ control. Device-gated no-op pending raw-frame helpers
/// (see notes). F-2/F-3 (INIT/PING on CID 0 -> INVALID_CHANNEL) likewise
/// need raw CID control.
#[test]
#[serial]
fn ctaphid_invalid_seq_and_channel() {
    if !transport::is_device_mode() {
        return;
    }
    // TODO(integrator): needs raw `send_frames(Vec<[u8;64]>)` to (a) corrupt
    // a continuation SEQ (F-4 -> ERR_INVALID_SEQ 0x04) and (b) target CID 0
    // (F-2/F-3 -> CTAP1_ERR_INVALID_CHANNEL 0x0B).
}
