//! nRF52840 NFCT Type 4A PICC, backed by Nordic's `libnfc_t4t.a`.
//!
//! The library runs in raw ISO-DEP mode: anti-collision, RATS/ATS, I-block
//! framing, chaining and WTX are handled inside the blob. APDUs arrive at
//! `t4t_callback` as `DATA_IND` events (possibly fragmented), are
//! reassembled into full C-APDUs, and pushed onto the apdu-dispatch
//! interchange — `ndef-app`, `fido-authenticator`, et al. handle them in
//! the idle loop.
//!
//! IRQ context (NFCT, priority 4) cannot call into the trussed stack
//! directly (trussed crypto runs at priority 2 and would deadlock), so the
//! IRQ stashes inbound APDUs into a single-slot mailbox and the idle loop
//! drains them via `fido_poll`.

use apdu_dispatch::interchanges;
use core::sync::atomic::{AtomicBool, Ordering};
use defmt::info;
use heapless::Vec;
use nrf_nfc::nfc_t4t as t4t;

// SAFETY (DATA_IND callback): runs in NFCT IRQ context. We touch only
// `cortex_m::interrupt::Mutex<RefCell<...>>`-protected statics and the
// `FIELD_ON` atomic. The `slice::from_raw_parts(data, data_length)` is sound
// only while we're inside this callback — t4t_lib owns the buffer and it
// stays valid until we return.
extern "C" fn t4t_callback(
    _ctx: *mut core::ffi::c_void,
    event: t4t::nfc_t4t_event_t,
    data: *const u8,
    data_length: usize,
    flags: u32,
) {
    info!(
        "t4t cb evt={=u32} len={=usize} flags={=u32:#x}",
        event, data_length, flags
    );
    match event {
        t4t::nfc_t4t_event_t_NFC_T4T_EVENT_FIELD_ON => {
            FIELD_ON.store(true, Ordering::Release);
            // Per-session reset: a partial APDU left over from a previous tap
            // (reader disconnects mid-ceremony) would otherwise prepend
            // garbage to the next session's first command.
            cortex_m::interrupt::free(|cs| {
                APDU_ACC.borrow(cs).borrow_mut().clear();
                *INCOMING.borrow(cs).borrow_mut() = None;
            });
            HAS_INCOMING.store(false, Ordering::Release);
        }
        t4t::nfc_t4t_event_t_NFC_T4T_EVENT_FIELD_OFF => {
            FIELD_ON.store(false, Ordering::Release);
        }
        t4t::nfc_t4t_event_t_NFC_T4T_EVENT_DATA_IND => {
            // Long C-APDUs arrive in fragments; concatenate until MORE=0.
            let more = (flags & t4t::nfc_t4t_data_ind_flags_t_NFC_T4T_DI_FLAG_MORE) != 0;
            let slice = if !data.is_null() && data_length > 0 {
                unsafe { core::slice::from_raw_parts(data, data_length) }
            } else {
                &[]
            };
            cortex_m::interrupt::free(|cs| {
                let mut acc = APDU_ACC.borrow(cs).borrow_mut();
                let _ = acc.extend_from_slice(slice);
                if !more {
                    let mut v: Vec<u8, MAX_APDU> = Vec::new();
                    let _ = v.extend_from_slice(&acc);
                    acc.clear();
                    *INCOMING.borrow(cs).borrow_mut() = Some(v);
                    HAS_INCOMING.store(true, Ordering::Release);
                }
            });
        }
        _ => {}
    }
}

// FIDO MakeCredential responses (CBOR + attestation cert + signature) hit
// ~1.5–2 KiB; size for headroom.
const TX_RESP_SIZE: usize = 4096;

// SAFETY (TX_RESP): writes are serialized below by `interrupt::free`. The
// t4t library reads asynchronously after `nfc_t4t_response_pdu_send` returns,
// but the reader side is synchronous (one APDU outstanding at a time), so
// back-to-back `send_response` calls don't race a still-in-flight prior TX.
static mut TX_RESP: [u8; TX_RESP_SIZE] = [0; TX_RESP_SIZE];

fn send_response(buf: &[u8]) {
    // Critical section: send_response is called from the idle loop, but the
    // NFCT IRQ may also touch shared state mid-call. Serializing keeps
    // TX_RESP consistent if a future code path ever re-enters.
    cortex_m::interrupt::free(|_cs| unsafe {
        let n = buf.len().min(TX_RESP_SIZE);
        let dst = core::ptr::addr_of_mut!(TX_RESP) as *mut u8;
        core::ptr::copy_nonoverlapping(buf.as_ptr(), dst, n);
        let _ = t4t::nfc_t4t_response_pdu_send(dst, n);
    });
}

// ── IRQ → idle bridge ────────────────────────────────────────────────────────

const MAX_APDU: usize = interchanges::SIZE;

// Single-slot mailbox: IRQ pushes one assembled APDU; idle drains it.
static INCOMING: cortex_m::interrupt::Mutex<core::cell::RefCell<Option<Vec<u8, MAX_APDU>>>> =
    cortex_m::interrupt::Mutex::new(core::cell::RefCell::new(None));

// Lock-free mirror of `INCOMING.is_some()`. `fido_poll` runs in a tight idle
// loop; reading this atomic on the common no-data path avoids a
// `cortex_m::interrupt::free` PRIMASK section, which would mask the priority-7
// NFCT IRQ and corrupt in-flight ISO-DEP frames (anti-collision/RATS fails).
static HAS_INCOMING: AtomicBool = AtomicBool::new(false);

// Reassembly buffer for fragmented DATA_IND chunks.
static APDU_ACC: cortex_m::interrupt::Mutex<core::cell::RefCell<Vec<u8, MAX_APDU>>> =
    cortex_m::interrupt::Mutex::new(core::cell::RefCell::new(Vec::new()));

/// `true` while a reader's RF field is up. The board's UserInterface impl
/// reads this so user-presence checks auto-approve during NFC sessions
/// (the tap itself is the consent signal — there's no button to press
/// while the phone is on the antenna).
pub static FIELD_ON: AtomicBool = AtomicBool::new(false);

pub fn field_on() -> bool {
    FIELD_ON.load(Ordering::Acquire)
}

/// Drain incoming APDU into apdu-dispatch and ship pending response back.
/// Must be called from idle every loop iteration.
/// The Nordic t4t lib drops a byte when fragmenting a response PDU that
/// exceeds one ISO-DEP frame (FSC = 256): apdu-dispatch serves the reader's
/// `Le`=256, so a 256-data + SW = 258-byte chunk overruns a single I-block
/// and the closed lib's fragmentation loses a byte at the boundary. Cap the
/// host's requested `Le` so every 61xx chunk (data + SW) fits one I-block;
/// the reader then GET-RESPONSEs for the rest. Short APDUs only (the t4t lib
/// can't parse extended-length requests anyway). lpc55/fm11nc08 is unaffected.
const NFC_MAX_LE: u8 = 250;

fn cap_le(apdu: &mut [u8]) {
    let n = apdu.len();
    let le_idx = if n == 5 {
        Some(4) // Case 2: header(4) + Le
    } else if n >= 7 && n == 6 + apdu[4] as usize {
        Some(n - 1) // Case 4: header(4) + Lc(1) + data + Le(1)
    } else {
        None // Case 1/3 (no Le) or a chained intermediate frame
    };
    if let Some(i) = le_idx {
        if apdu[i] == 0 || apdu[i] > NFC_MAX_LE {
            apdu[i] = NFC_MAX_LE;
        }
    }
}

pub fn fido_poll(rq: &mut interchanges::Requester<'static>) {
    // Common no-data path is a single atomic load — no PRIMASK section. Only
    // enter `cortex_m::interrupt::free` (which masks the priority-7 NFCT IRQ)
    // when there's actually an APDU to take, so the tight idle-loop poll never
    // stalls in-flight ISO-DEP frames.
    let has_data = HAS_INCOMING.load(Ordering::Acquire);
    let pending = if has_data {
        cortex_m::interrupt::free(|cs| {
            HAS_INCOMING.store(false, Ordering::Release);
            INCOMING.borrow(cs).borrow_mut().take()
        })
    } else {
        None
    };
    if let Some(mut apdu) = pending {
        cap_le(&mut apdu);
        info!(
            "fido_poll: pushing apdu len={=usize} bytes={=[u8]:#x}",
            apdu.len(),
            apdu.as_slice()
        );
        if let Ok(data) = interchanges::Data::from_slice(&apdu) {
            match rq.request(data) {
                Ok(_) => info!("fido_poll: rq.request OK"),
                Err(_) => info!("fido_poll: rq.request FAILED (busy)"),
            }
        }
    }
    if rq.state() == interchange::State::Responded {
        if let Some(resp) = rq.take_response() {
            info!("fido_poll: sending response len={=usize}", resp.len());
            send_response(&resp);
        }
    }
}

pub struct NfctDevice {
    _private: (),
}

impl NfctDevice {
    /// Bring up the t4t library. NFCID1 is derived inside the library (via
    /// `nfc_platform_nfcid1_default_bytes_get`); the seed argument is unused.
    pub fn new(_nfct: nrf52840_pac::NFCT, _uid_seed: [u8; 6]) -> Self {
        // ── Errata 57 — NFCT modulation amplitude tuning ─────────────────────
        // Nordic considers this errata "officially nRF52832 only" so neither
        // nrfx_nfct nor libnfc_t4t apply it for nRF52840. Empirically it is
        // required: without these pokes the SENSRES (ATQA) reply goes out at
        // insufficient modulation depth and aggressive readers miss it
        // and abandon anti-collision.
        //
        // Sequence: TASKS_DISABLE → ~150 nops settling time → 4 undocumented
        // analog tuning registers. Library will TASKS_SENSE later via
        // emulation_start. The pokes survive subsequent activate cycles.
        unsafe {
            core::ptr::write_volatile(0x4000_5004 as *mut u32, 0x0000_0001); // TASKS_DISABLE
            for _ in 0..150 {
                cortex_m::asm::nop();
            }
            core::ptr::write_volatile(0x4000_5610 as *mut u32, 0x0000_0005);
            core::ptr::write_volatile(0x4000_5614 as *mut u32, 0x0000_003F);
            core::ptr::write_volatile(0x4000_5618 as *mut u32, 0x0000_0000);
            core::ptr::write_volatile(0x4000_5688 as *mut u32, 0x0000_0001);
        }

        let rc = unsafe { t4t::nfc_t4t_setup(Some(t4t_callback), core::ptr::null_mut()) };
        if rc != 0 {
            info!("nfct: nfc_t4t_setup rc={}", rc);
            return Self { _private: () };
        }

        // 4-byte NFCID1 (single cascade). Some readers treat 4-byte UID tags
        // as NTAG-class and auto-read NDEF; with 7-byte UIDs they probe for
        // FIDO and DESELECT if not satisfied (observed empirically). Passing
        // the length value as the parameter payload tells the library to use
        // a default-derived NFCID1 of that length.
        let mut nfcid_len: u8 = 4;
        let _ = unsafe {
            t4t::nfc_t4t_parameter_set(
                t4t::nfc_t4t_param_id_t_NFC_T4T_PARAM_NFCID1,
                &mut nfcid_len as *mut u8 as *mut core::ffi::c_void,
                1,
            )
        };

        // Raise the Frame-Waiting-Time ceiling to the NFC max (FWI_MAX = 8)
        // so the library's WTX can hold a strict PC/SC reader through slow
        // on-card crypto (ES256 keygen/sign; ML-DSA is ~100s of ms). Without
        // it the reader times out mid-MakeCredential ("transaction failed");
        // a phone is lenient. Payload is the fwi_max enum byte.
        let mut fwi_max: u8 = t4t::nfc_t4t_fwi_max_val_t_NFC_T4T_FWI_MAX_VAL_NFC as u8;
        let _ = unsafe {
            t4t::nfc_t4t_parameter_set(
                t4t::nfc_t4t_param_id_t_NFC_T4T_PARAM_FWI_MAX,
                &mut fwi_max as *mut u8 as *mut core::ffi::c_void,
                1,
            )
        };

        // Force the SAK "Protocol" bits to T4AT (ISO 14443-4) before
        // starting emulation. `nfc_t4t_setup` doesn't set this and the
        // chip's reset value advertises a cascade-tag bit on a 4-byte
        // UID, which iPhone treats as a malformed NFC-A tag and stops
        // chunked NDEF reads after the first ReadBinary.
        {
            use nrf_nfc::nrfx_nfct::{
                nrf_nfct_selres_protocol_t_NRF_NFCT_SELRES_PROTOCOL_T4AT,
                nrfx_nfct_param_id_t_NRFX_NFCT_PARAM_ID_SEL_RES, nrfx_nfct_param_t,
                nrfx_nfct_param_t__bindgen_ty_1, nrfx_nfct_parameter_set,
            };
            let p = nrfx_nfct_param_t {
                id: nrfx_nfct_param_id_t_NRFX_NFCT_PARAM_ID_SEL_RES,
                data: nrfx_nfct_param_t__bindgen_ty_1 {
                    sel_res_protocol: nrf_nfct_selres_protocol_t_NRF_NFCT_SELRES_PROTOCOL_T4AT
                        as u8,
                },
            };
            let rc = unsafe { nrfx_nfct_parameter_set(&p) };
            if rc != 0 {
                info!("nfct: SEL_RES set rc={}", rc);
            }
        }

        // Raw ISO-DEP mode — DATA_IND callbacks deliver APDU fragments.
        // (Skipping ndef_*_payload_set selects raw mode in the library.)

        let rc = unsafe { t4t::nfc_t4t_emulation_start() };
        if rc != 0 {
            info!("nfct: emulation_start rc={}", rc);
        }

        Self { _private: () }
    }
}
