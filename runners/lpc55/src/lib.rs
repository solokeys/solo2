#![no_std]
include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

// panic handler, depending on debug/release build
// BUT: need to run in release anyway, to have USB work
use panic_halt as _;
// use panic_semihosting as _;

use board::clock_controller;
pub use board::hal;
use usb_device::device::UsbVidPid; // re-export for convenience

#[allow(unused_imports)]
use hal::drivers::timer::Elapsed;

use types::Board;

#[macro_use]
extern crate delog;
generate_macros!();

pub mod initializer;
pub mod types;

// Logging
#[derive(Debug)]
pub struct Flusher {}

impl delog::Flusher for Flusher {
    fn flush(&self, _logs: &str) {
        #[cfg(feature = "log-rtt")]
        rtt_target::rprint!(_logs);

        #[cfg(feature = "log-semihosting")]
        cortex_m_semihosting::hprint!(_logs).ok();

        #[cfg(feature = "log-serial")]
        // see https://git.io/JLARR for the plan on how to improve this once we switch to RTIC 0.6
        rtic::pend(hal::raw::Interrupt::MAILBOX);
    }
}

// delog!(Delogger, 16*1024, 3*1024, Flusher);
delog!(Delogger, 1, 2048, Flusher);

#[cfg(any(
    feature = "log-semihosting",
    feature = "log-serial",
    feature = "log-rtt"
))]
static FLUSHER: Flusher = Flusher {};

// TODO: move board-specifics to BSPs
pub fn init_board(
    device_peripherals: hal::raw::Peripherals,
) -> (
    // types::Authenticator,
    types::ApduDispatch,
    types::CtaphidDispatch,
    types::Trussed,
    types::Apps,
    Option<types::UsbClasses>,
    Option<types::Iso14443>,
    types::PerformanceTimer,
    Option<clock_controller::DynamicClockController>,
    types::NfcWaitExtender,
) {
    #[cfg(feature = "log-rtt")]
    rtt_target::rtt_init_print!();

    #[cfg(any(
        feature = "log-semihosting",
        feature = "log-serial",
        feature = "log-rtt"
    ))]
    Delogger::init_default(delog::LevelFilter::Debug, &FLUSHER).ok();

    info_now!(
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
        require_prince: require_prince,
        boot_to_bootrom: true,
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
    info_now!("got initializer");

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

    #[cfg(feature = "provisioner-app")]
    let store = everything.filesystem.store.clone();
    #[cfg(feature = "provisioner-app")]
    let internal_fs = everything.filesystem.internal_storage_fs;

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
    );

    (
        everything.interfaces.apdu_dispatch,
        everything.interfaces.ctaphid_dispatch,
        everything.trussed,
        apps,
        everything.usb.usb_classes,
        everything.nfc.iso14443,
        everything.basic.perf_timer,
        clock_controller,
        everything.basic.delay_timer,
    )
}
