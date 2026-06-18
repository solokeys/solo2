// SPDX-License-Identifier: MIT
//
//! Type 4 Tag library — FFI to Nordic's closed-source `libnfc_t4t.a`
//! (vendored at `vendor/lib/libnfc_t4t.a`) plus the Rust platform-glue
//! the library calls back into.
//!
//! The public T4T API bindings are pre-generated and committed at
//! `vendor/nfc_t4t_bindings.rs`.
//!
//! `libnfc_t4t.a` links against six C-ABI symbols (per
//! `nm vendor/lib/libnfc_t4t.a | grep ' U '`):
//!
//!   nfc_platform_setup
//!   nfc_platform_nfcid1_default_bytes_get
//!   nfc_platform_event_handler
//!   nfc_platform_cb_request
//!   nfc_platform_buffer_alloc
//!   nfc_platform_buffer_free
//!
//! Modeled on Nordic's NCS reference (`nrf/subsys/nfc/lib/platform.c`,
//! BSD-3) without the Zephyr clock-control dependency: HFXO is started
//! once at boot in the runner and stays on, so we drive ACTIVATE
//! synchronously when a field is detected.
//!
//! The shims run in NFCT IRQ context (priority 4); the library trusts
//! pointer validity, and the buffer pool is single-flighted via the
//! in-use flag.

include!("../vendor/nfc_t4t_bindings.rs");

// Each `pub unsafe extern "C" fn` below is an FFI entry point invoked
// solely by libnfc_t4t.a. The safety contract is fixed by the .a's
// calling code.
#[allow(clippy::missing_safety_doc)]
mod platform {
    use core::ffi::c_void;
    use core::ptr;
    use core::sync::atomic::{AtomicBool, Ordering};

    use crate::nrfx_nfct::{
        nrfx_nfct_evt_id_t_NRFX_NFCT_EVT_FIELD_DETECTED,
        nrfx_nfct_evt_id_t_NRFX_NFCT_EVT_FIELD_LOST, nrfx_nfct_evt_t, nrfx_nfct_state_force,
        nrfx_nfct_state_t_NRFX_NFCT_STATE_ACTIVATED,
    };

    /// Library callback resolver — libnfc_t4t.a hands us a function pointer
    /// of this shape during `nfc_platform_setup`; we invoke it from
    /// `nfc_platform_cb_request`.
    pub type nfc_lib_cb_resolve_t = unsafe extern "C" fn(p_ctx: *const c_void, p_data: *const u8);

    // ── Per-tag DMA buffer pool (Type 4 Tag) ───────────────────────────────
    //
    // `NFC_PLATFORM_T4T_BUFFER_SIZE = 259` from nfc_platform.h — sized to
    // hold FSD=256 plus a 3-byte WTX frame. We give the library twice that
    // for its TX+RX areas and single-flight allocations: the library owns
    // the whole pool or none of it.
    //
    // 4-byte alignment for the EasyDMA-backed NFCT engine. The peripheral's
    // register reference is silent on alignment, but every other EasyDMA
    // peripheral on this part requires word alignment, so we honor the
    // stricter constraint.

    const T4T_BUFFER_SIZE: usize = 259;
    const T4T_TOTAL_BUF: usize = 2 * T4T_BUFFER_SIZE;

    #[repr(align(4))]
    struct AlignedBuf([u8; T4T_TOTAL_BUF]);

    static mut S_T4T_BUF: AlignedBuf = AlignedBuf([0u8; T4T_TOTAL_BUF]);
    static S_BUF_IN_USE: AtomicBool = AtomicBool::new(false);

    // Set by `nfc_platform_setup`, read by `nfc_platform_cb_request`.
    // Single-writer (setup runs once at boot, in thread mode), single-reader
    // (cb_request runs in IRQ).
    static mut S_CB_RESOLVE: Option<nfc_lib_cb_resolve_t> = None;

    // ── nfc_platform_setup ─────────────────────────────────────────────────

    #[no_mangle]
    pub unsafe extern "C" fn nfc_platform_setup(
        nfc_lib_cb_resolve: Option<nfc_lib_cb_resolve_t>,
        p_irq_priority: *mut u8,
    ) -> i32 {
        if nfc_lib_cb_resolve.is_none() || p_irq_priority.is_null() {
            return -22; // -EINVAL
        }
        S_CB_RESOLVE = nfc_lib_cb_resolve;
        *p_irq_priority = 6;
        defmt::info!("nfc_platform: setup OK");
        0
    }

    // ── nfc_platform_nfcid1_default_bytes_get ──────────────────────────────
    //
    // Derive default NFCID1 bytes from `FICR.DEVICEADDR` (universally
    // present; FICR.NFC.TAGHEADER doesn't exist on every SoC). Byte 0 =
    // 0x04 (NXP cascade tag) — without it iPhone classifies the device as
    // a generic "fake" tag. Length must be 4, 7, or 10 per ISO 14443-3
    // NFCID1 sizes.

    const FICR_DEVICEADDR0: *const u32 = 0x1000_0060 as *const u32;

    #[no_mangle]
    pub unsafe extern "C" fn nfc_platform_nfcid1_default_bytes_get(
        p_buf: *mut u8,
        buf_len: u32,
    ) -> i32 {
        if p_buf.is_null() {
            return -22; // -EINVAL
        }
        if buf_len != 4 && buf_len != 7 && buf_len != 10 {
            return -7; // -E2BIG
        }
        let a = ptr::read_volatile(FICR_DEVICEADDR0);
        let b = ptr::read_volatile(FICR_DEVICEADDR0.offset(1));

        *p_buf.offset(0) = 0x04;
        *p_buf.offset(1) = (a >> 24) as u8;
        *p_buf.offset(2) = (a >> 16) as u8;
        *p_buf.offset(3) = (a >> 8) as u8;

        if buf_len >= 7 {
            *p_buf.offset(4) = (b >> 24) as u8;
            *p_buf.offset(5) = (b >> 16) as u8;
            *p_buf.offset(6) = (b >> 8) as u8;
            if buf_len == 10 {
                *p_buf.offset(7) = b as u8;
                *p_buf.offset(8) = a as u8;
                *p_buf.offset(9) = ((a >> 16) ^ (b >> 16)) as u8;
            }
        }
        defmt::info!(
            "nfc_platform: nfcid_default len={} byte0={=u8:#x}",
            buf_len,
            *p_buf
        );
        0
    }

    // ── nfc_platform_event_handler ─────────────────────────────────────────
    //
    // libnfc_t4t.a forwards a subset of nrfx events so the platform can
    // react to field transitions:
    //
    //  1. FIELD_DETECTED — drive NFCT to ACTIVATED. The library leaves this
    //     to the platform because the NCS reference gates it on HFXO
    //     becoming ready; our HFXO is permanently on, so we activate
    //     synchronously.
    //
    //  2. FIELD_LOST — restore FRAMEDELAYMAX to the spec default. The
    //     library widens it during ISO-DEP; without restoring, the next
    //     session's anti-coll runs in too wide a window and readers reject
    //     the response.

    // FRAMEDELAYMAX register, fixed by the SVD; see nRF52840 product
    // spec §6.18 (NFCT).
    const NRF_NFCT_FRAMEDELAYMAX: *mut u32 = 0x4000_5508 as *mut u32;
    const FRAMEDELAYMAX_SPEC_DEFAULT: u32 = 0x1000;

    #[no_mangle]
    pub unsafe extern "C" fn nfc_platform_event_handler(p_event: *const nrfx_nfct_evt_t) {
        let id = (*p_event).evt_id;
        if id == nrfx_nfct_evt_id_t_NRFX_NFCT_EVT_FIELD_DETECTED {
            defmt::info!("nfc_platform: FIELD_DETECTED");
            nrfx_nfct_state_force(nrfx_nfct_state_t_NRFX_NFCT_STATE_ACTIVATED);
        } else if id == nrfx_nfct_evt_id_t_NRFX_NFCT_EVT_FIELD_LOST {
            defmt::info!("nfc_platform: FIELD_LOST (FRAMEDELAYMAX restored)");
            ptr::write_volatile(NRF_NFCT_FRAMEDELAYMAX, FRAMEDELAYMAX_SPEC_DEFAULT);
        } else {
            defmt::info!("nfc_platform: nrfx evt {=u32:#x}", id);
        }
    }

    // ── nfc_platform_cb_request ────────────────────────────────────────────
    //
    // libnfc_t4t.a uses this to dispatch deferred callbacks. We invoke the
    // resolver directly (no scheduling), so the library callback runs in
    // the NFCT IRQ.

    #[no_mangle]
    pub unsafe extern "C" fn nfc_platform_cb_request(
        p_ctx: *const c_void,
        _ctx_len: usize,
        p_data: *const u8,
        _data_len: usize,
        _copy_data: bool,
    ) {
        defmt::info!(
            "nfc_platform: cb_request ctx={=u32:#x} data={=u32:#x}",
            p_ctx as u32,
            p_data as u32
        );
        if let Some(resolve) = S_CB_RESOLVE {
            resolve(p_ctx, p_data);
        }
    }

    // ── nfc_platform_buffer_alloc / free ───────────────────────────────────

    #[no_mangle]
    pub unsafe extern "C" fn nfc_platform_buffer_alloc(size: usize) -> *mut u8 {
        let in_use = S_BUF_IN_USE.load(Ordering::SeqCst);
        defmt::info!(
            "nfc_platform: buf_alloc size={=usize} in_use={=bool}",
            size,
            in_use
        );
        if in_use || size > T4T_TOTAL_BUF {
            return ptr::null_mut();
        }
        S_BUF_IN_USE.store(true, Ordering::SeqCst);
        core::ptr::addr_of_mut!(S_T4T_BUF.0) as *mut u8
    }

    #[no_mangle]
    pub unsafe extern "C" fn nfc_platform_buffer_free(p_buf: *mut u8) {
        defmt::info!("nfc_platform: buf_free p={=u32:#x}", p_buf as u32);
        let our_buf = core::ptr::addr_of_mut!(S_T4T_BUF.0) as *mut u8;
        if p_buf == our_buf {
            S_BUF_IN_USE.store(false, Ordering::SeqCst);
        }
    }
}
