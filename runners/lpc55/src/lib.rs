#![no_std]
include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

// panic handler, depending on debug/release build
// BUT: need to run in release anyway, to have USB work
use panic_halt as _;
// use panic_semihosting as _;

use board::clock_controller;
pub use board::hal;
use defmt::info;
use delog::delog;
use usb_device::device::UsbVidPid; // re-export for convenience

#[allow(unused_imports)]
use hal::drivers::timer::Elapsed;

pub mod device_config;
pub mod initializer;
pub mod types;

// Logging
#[derive(Debug)]
pub struct Flusher {}

impl delog::Flusher for Flusher {
    fn flush(&self, _logs: &str) {
        #[cfg(feature = "log-defmt")]
        defmt::println!("Forwarded delog logs:\n{}", _logs);
    }
}

// delog!(Delogger, 16*1024, 3*1024, Flusher);
delog!(Delogger, 4096, 2048, Flusher);

#[cfg(feature = "log-defmt")]
static FLUSHER: Flusher = Flusher {};

// ── Wallet consent: runner-driven, non-blocking user-presence ────────────────
//
// A wallet sign arms `wallet_app::consent` (request) and polls the result; the
// idle loop calls `confirm_user_present_non_blocking(now_ms)` once per pass to fill the result from
// the idle-reachable inputs (monotonic clock + NFC field). This is the
// non-blocking analogue of `check_user_presence` for the wallet path only
// (FIDO keeps using `check_user_presence`).
//
// A button gesture latched by the idle `board::trussed::poll_buttons` is
// consumed here too: any committed press (Approve/Strong) grants.
//
// Thin wrapper over the shared `board::trussed::read_presence`. lpc55 has no
// explicit-deny button, so the only outcomes are Grant/Pending. The non-blocking
// wallet path passes `has_buttons = true` (the no-buttons auto-approve only
// applies to the blocking FIDO path) and the NDEF suppression flag (a phone tap
// during a sign reads the override URL instead of granting).
#[cfg(feature = "wallet")]
pub fn confirm_user_present_non_blocking(now_ms: u32) {
    use board::trussed::{begin_button_request, poll_buttons, read_presence, Presence};
    use core::sync::atomic::{AtomicU32, Ordering};
    use wallet_app::consent;

    // First-seen `now_ms` of the in-flight request; `u32::MAX` = none (sentinel).
    static CONSENT_START: AtomicU32 = AtomicU32::new(u32::MAX);

    if !consent::is_up_requested() {
        CONSENT_START.store(u32::MAX, Ordering::Relaxed);
        return;
    }

    let (start, new_request) = match CONSENT_START.compare_exchange(
        u32::MAX,
        now_ms,
        Ordering::Relaxed,
        Ordering::Relaxed,
    ) {
        Ok(_) => (now_ms, true),
        Err(existing) => (existing, false),
    };

    if new_request {
        begin_button_request();
        // FIDO_OVER_NFC latches the transport of the last FIDO dispatch and is
        // only cleared by the next USB/contact FIDO message — the wallet HID
        // never passes through those, so a contactless FIDO op earlier in the
        // power session would auto-grant this sign. Clear it at the start of
        // the consent window; a live tap during the window re-sets it.
        board::trussed::FIDO_OVER_NFC.store(false, core::sync::atomic::Ordering::Relaxed);
    }
    poll_buttons();

    #[cfg(feature = "ndef-app")]
    let suppressed = ndef_app::has_override();
    #[cfg(not(feature = "ndef-app"))]
    let suppressed = false;

    match read_presence(true, suppressed) {
        Presence::Grant(_) => consent::set_up_result(consent::GRANTED),
        Presence::Pending => {
            if now_ms.wrapping_sub(start) > 30_000 {
                consent::set_up_result(consent::TIMED_OUT);
            } else {
                consent::set_up_result(consent::WAITING);
            }
        }
    }
}

// TODO: move board-specifics to BSPs
#[allow(clippy::type_complexity)]
pub fn init_board(
    device_peripherals: hal::raw::Peripherals,
) -> (
    // types::Authenticator,
    types::ApduDispatch,
    types::CtaphidDispatch,
    types::Trussed,
    types::Apps,
    types::WalletSlot,
    Option<types::UsbClasses>,
    Option<types::Iso14443>,
    types::PerformanceTimer,
    Option<clock_controller::DynamicClockController>,
    types::NfcWaitExtender,
) {
    #[cfg(feature = "log-defmt")]
    Delogger::init_default(delog::LevelFilter::Debug, &FLUSHER).ok();

    info!(
        "entering init_board {}.{}.{}",
        build_constants::CARGO_PKG_VERSION_MAJOR,
        build_constants::CARGO_PKG_VERSION_MINOR,
        build_constants::CARGO_PKG_VERSION_PATCH
    );

    #[cfg(not(feature = "no-encrypted-storage"))]
    let require_prince = true;
    #[cfg(feature = "no-encrypted-storage")]
    let require_prince = false;

    let config = initializer::Config {
        secure_firmware_version: Some(build_constants::CARGO_PKG_VERSION),
        nfc_enabled: true,
        require_prince,
        boot_to_bootrom: true,
        // USB identity: the standard SoloKeys Solo 2 (VID 0x1209), product
        // string from the PFR. A user-supplied vid/pid (persisted on the FS,
        // which mounts before USB) can override this later; the default is
        // SoloKeys for every build.
        usb_config: Some(initializer::UsbConfig {
            manufacturer_name: "SoloKeys",
            product_name: initializer::UsbProductName::UsePfr,
            vid_pid: UsbVidPid(0x1209, 0xbeee),
        }),
    };

    let mut initializer = initializer::Initializer::new(
        config,
        hal::Syscon::from(device_peripherals.SYSCON),
        hal::Pmc::from(device_peripherals.PMC),
        hal::Anactrl::from(device_peripherals.ANACTRL),
    );
    info!("got initializer");

    let mut everything = initializer.initialize_all(
        hal::Iocon::from(device_peripherals.IOCON),
        hal::Gpio::from(device_peripherals.GPIO),
        hal::Adc::from(device_peripherals.ADC0),
        hal::Dma::from(device_peripherals.DMA0),
        hal::peripherals::ctimer::Ctimer0::from(device_peripherals.CTIMER0),
        hal::peripherals::ctimer::Ctimer1::from(device_peripherals.CTIMER1),
        hal::peripherals::ctimer::Ctimer2::from(device_peripherals.CTIMER2),
        hal::peripherals::ctimer::Ctimer3::from(device_peripherals.CTIMER3),
        hal::peripherals::ctimer::Ctimer4::from(device_peripherals.CTIMER4),
        hal::Pfr::new(),
        hal::peripherals::flexcomm::Flexcomm0::from((
            device_peripherals.FLEXCOMM0,
            device_peripherals.I2C0,
            device_peripherals.I2S0,
            device_peripherals.SPI0,
            device_peripherals.USART0,
        )),
        // The FM11 NFC I2C bus is on a different Flexcomm per board: FC1 on the EVK,
        // FC4 on the solo board.
        #[cfg(feature = "board-lpcxpresso55")]
        hal::peripherals::flexcomm::Flexcomm1::from((
            device_peripherals.FLEXCOMM1,
            device_peripherals.I2C1,
            device_peripherals.I2S1,
            device_peripherals.SPI1,
            device_peripherals.USART1,
        )),
        #[cfg(not(feature = "board-lpcxpresso55"))]
        hal::peripherals::flexcomm::Flexcomm4::from((
            device_peripherals.FLEXCOMM4,
            device_peripherals.I2C4,
            device_peripherals.I2S4,
            device_peripherals.SPI4,
            device_peripherals.USART4,
        )),
        hal::InputMux::from(device_peripherals.INPUTMUX),
        hal::Pint::from(device_peripherals.PINT),
        hal::Usbhs::from((
            device_peripherals.USBPHY,
            device_peripherals.USB1,
            device_peripherals.USBHSH,
        )),
        hal::Usbfs::from((device_peripherals.USB0, device_peripherals.USBFSH)),
        hal::Rng::from(device_peripherals.RNG),
        hal::Prince::from(device_peripherals.PRINCE),
        hal::Flash::from(device_peripherals.FLASH),
        hal::Rtc::from(device_peripherals.RTC),
    );

    let _is_passive_mode = initializer.is_in_passive_operation(&everything.clock);
    let clock_controller =
        initializer.get_dynamic_clock_control(&mut everything.clock, &mut everything.basic);

    // rgb.turn_off();
    info!(
        "init took {} ms",
        everything.basic.perf_timer.elapsed().0 / 1000
    );

    #[cfg(any(
        feature = "provisioner-app",
        feature = "oath-export",
        feature = "admin-app"
    ))]
    let store = everything.filesystem.store;
    #[cfg(feature = "provisioner-app")]
    let internal_fs = everything.filesystem.internal_storage_fs;

    // Run migrations on persistent state before any app touches the filesystem.
    // Idempotent: safe to call on every boot, no-op on already-migrated state
    // (and on fresh devices where the relevant directories do not exist yet).
    #[cfg(feature = "fido-authenticator")]
    {
        use trussed::store::Store as _;
        let _ = fido_authenticator::state::migrate::migrate_no_rp_dir(
            everything.filesystem.store.ifs(),
            littlefs2::path!("fido/dat"),
        );
    }

    let apps = types::Apps::new(
        &mut everything.trussed,
        #[cfg(feature = "provisioner-app")]
        {
            types::ProvisionerNonPortable {
                store,
                stolen_filesystem: unsafe { &mut *internal_fs },
                nfc_powered: _is_passive_mode,
            }
        },
        #[cfg(any(feature = "admin-app", feature = "oath-export"))]
        store,
    );

    // Wallet lives outside `Apps`: its sign path blocks on user presence and
    // must not hold the shared `apps` lock for the duration. Present only when
    // USB came up (passive/NFC-only boot has no WalletHid responder).
    #[cfg(feature = "wallet")]
    let wallet = everything
        .usb
        .wallet_responder
        .take()
        .map(|responder| types::Wallet::new(&mut everything.trussed, responder));
    #[cfg(not(feature = "wallet"))]
    let wallet: types::WalletSlot = ();

    // A single button owner serves both idle's non-blocking wallet consent and
    // Trussed's blocking FIDO consent loop.
    let buttons = everything.basic.three_buttons.take();
    board::trussed::install_buttons(buttons);

    (
        everything.interfaces.apdu_dispatch,
        everything.interfaces.ctaphid_dispatch,
        everything.trussed,
        apps,
        wallet,
        everything.usb.usb_classes,
        everything.nfc.iso14443,
        everything.basic.perf_timer,
        clock_controller,
        everything.basic.delay_timer,
    )
}
