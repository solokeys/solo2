//! Transport abstraction so the wallet integration tests run unchanged on two
//! backends:
//!
//! - **sim** (default): drives `wallet_app::Authenticator::respond()` against
//!   the in-process Trussed runtime (`super::sim`). No USB, runs in CI.
//! - **device** (`WALLET_BACKEND=device`): frames each APDU over the Ledger-
//!   style USB HID transport (vendor usage page `0xFFA0`) to a real LPC55 /
//!   EVK. Requires the wallet built in (enumerates as Ledger Nano S).
//!
//! User presence is handled by `super::up` (already dual-backend: in-process
//! `solo_pc::buttons` for sim, `probe-rs` `UP_CONTROL` writes for device — set
//! `UP_BACKEND=probe-rs UP_CONTROL_ADDR=0x20043bfc PROBE_RS_CHIP=LPC55S69JBD100`).
//!
//! A test body is `FnOnce(&mut dyn Wallet)`; `with_wallet` picks the backend.

use std::time::{Duration, Instant};

use iso7816::{Command, Data};
use wallet_app::Authenticator;

/// One wallet APDU round-trip. `Ok(body)` on `0x9000`, else `Err(sw)`.
pub trait Wallet {
    fn transact(&mut self, ins: u8, p1: u8, p2: u8, data: &[u8]) -> Result<Vec<u8>, u16>;
}

/// Build a wallet APDU (proprietary CLA `0xE0`), short or extended Lc.
fn build_apdu(ins: u8, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
    let mut b = vec![0xE0u8, ins, p1, p2];
    if data.is_empty() {
        // no Lc
    } else if data.len() <= 255 {
        b.push(data.len() as u8);
        b.extend_from_slice(data);
    } else {
        b.push(0x00);
        b.extend_from_slice(&(data.len() as u16).to_be_bytes());
        b.extend_from_slice(data);
    }
    b
}

pub fn is_device_mode() -> bool {
    std::env::var("WALLET_BACKEND").as_deref() == Ok("device")
}

/// Run `f` against the active backend, bracketed by a sticky UP approve (so
/// gated ops pass unless a test explicitly denies) and a UP reset.
pub fn with_wallet<F>(f: F)
where
    F: FnOnce(&mut dyn Wallet) + Send,
{
    super::up::reset();
    super::up::approve_sticky();
    if is_device_mode() {
        let mut w = DeviceWallet::open();
        f(&mut w);
    } else {
        super::sim::with_client(|client| {
            let mut w = SimWallet {
                auth: Authenticator::new(client),
            };
            f(&mut w);
        });
    }
    super::up::reset();
}

// ── sim backend ──────────────────────────────────────────────────────────────

struct SimWallet<'a> {
    auth: Authenticator<super::sim::TestClient<'a>>,
}

impl Wallet for SimWallet<'_> {
    fn transact(&mut self, ins: u8, p1: u8, p2: u8, data: &[u8]) -> Result<Vec<u8>, u16> {
        let bytes = build_apdu(ins, p1, p2, data);
        let cmd = Command::<4608>::try_from(bytes.as_slice()).map_err(|_| 0x6700u16)?;
        let mut reply = Data::<512>::new();
        match self.auth.respond(&cmd, &mut reply) {
            Ok(()) => Ok(reply.as_slice().to_vec()),
            Err(status) => Err(u16::from(status)),
        }
    }
}

// ── device backend (Ledger-style HID, usage page 0xFFA0) ─────────────────────

const VID: u16 = 0x1209; // SoloKeys default; the device is matched by usage page first
const USAGE_PAGE: u16 = 0xFFA0;
const PACKET_SIZE: usize = 64;
const TRANSPORT_HEADER: [u8; 3] = [0x01, 0x01, 0x05];

struct DeviceWallet {
    dev: hidapi::HidDevice,
}

impl DeviceWallet {
    fn open() -> Self {
        let api = hidapi::HidApi::new().expect("init HID API");
        let list: Vec<_> = api.device_list().collect();
        // Prefer the 0xFFA0 (wallet HID) interface; fall back to any matching-
        // VID interface (hidapi returns usage_page 0 for some macOS backends).
        let info = list
            .iter()
            .find(|d| d.usage_page() == USAGE_PAGE)
            .or_else(|| list.iter().find(|d| d.vendor_id() == VID))
            .expect("no wallet HID interface (usage 0xFFA0 / VID 0x2c97) — is the wallet build flashed?");
        let dev = api.open_path(info.path()).expect("open wallet HID");
        Self { dev }
    }
}

impl Wallet for DeviceWallet {
    fn transact(&mut self, ins: u8, p1: u8, p2: u8, data: &[u8]) -> Result<Vec<u8>, u16> {
        let apdu = build_apdu(ins, p1, p2, data);

        // Frame the APDU into 64-byte HID packets (seq 0 carries the 2-byte
        // total length); write each with the macOS report-id 0 prefix.
        let total = apdu.len();
        let mut seq: u16 = 0;
        let mut off = 0usize;
        loop {
            let mut frame = Vec::with_capacity(PACKET_SIZE + 1);
            frame.push(0x00); // report id
            frame.extend_from_slice(&TRANSPORT_HEADER);
            frame.extend_from_slice(&seq.to_be_bytes());
            if seq == 0 {
                frame.extend_from_slice(&(total as u16).to_be_bytes());
            }
            let room = PACKET_SIZE + 1 - frame.len();
            let end = (off + room).min(total);
            frame.extend_from_slice(&apdu[off..end]);
            off = end;
            frame.resize(PACKET_SIZE + 1, 0);
            self.dev.write(&frame).expect("HID write");
            seq += 1;
            if off >= total {
                break;
            }
        }

        // Reassemble the response.
        let deadline = Instant::now() + Duration::from_secs(40);
        let mut payload: Vec<u8> = Vec::new();
        let mut expected: Option<usize> = None;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                panic!("HID read timeout");
            }
            let mut pkt = [0u8; PACKET_SIZE];
            let n = self
                .dev
                .read_timeout(&mut pkt, remaining.as_millis() as i32)
                .expect("HID read");
            if n == 0 || pkt[..3] != TRANSPORT_HEADER {
                continue;
            }
            let pkt_seq = u16::from_be_bytes([pkt[3], pkt[4]]);
            if pkt_seq == 0 {
                let len = u16::from_be_bytes([pkt[5], pkt[6]]) as usize;
                expected = Some(len);
                payload.extend_from_slice(&pkt[7..PACKET_SIZE]);
            } else {
                payload.extend_from_slice(&pkt[5..PACKET_SIZE]);
            }
            if let Some(len) = expected {
                if payload.len() >= len {
                    payload.truncate(len);
                    break;
                }
            }
        }

        if payload.len() < 2 {
            panic!("short wallet response: {payload:02x?}");
        }
        let sw = u16::from_be_bytes([payload[payload.len() - 2], payload[payload.len() - 1]]);
        let body = payload[..payload.len() - 2].to_vec();
        if sw == 0x9000 {
            Ok(body)
        } else {
            Err(sw)
        }
    }
}
