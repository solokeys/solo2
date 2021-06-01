#![no_std]
include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

// panic handler, depending on debug/release build
// BUT: need to run in release anyway, to have USB work
use panic_halt as _;
use c_stubs as _;

use usb_device::device::UsbVidPid;
use board::clock_controller;
pub use board::hal; // re-export for convenience

#[allow(unused_imports)]
use hal::drivers::timer::Elapsed;

use types::Board;

#[macro_use]
extern crate delog;
generate_macros!();

pub mod types;
pub mod initializer;


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

delog!(Delogger, 16*1024, 3*1024, Flusher);
static FLUSHER: Flusher = Flusher {};

// TODO: move board-specifics to BSPs
pub fn init_board(
    device_peripherals: hal::raw::Peripherals,
    core_peripherals: rtic::Peripherals,
) -> (
    // types::Authenticator,
    types::ApduDispatch,
    types::CtaphidDispatch,
    types::Trussed,

    types::Apps,

    Option<types::UsbClasses>,
    Option<types::Iso14443>,

    types::PerfTimer,
    Option<clock_controller::DynamicClockController>,
    types::HwScheduler,
) {
    #[cfg(feature = "log-rtt")]
    rtt_target::rtt_init_print!();

    Delogger::init_default(delog::LevelFilter::Debug, &FLUSHER).ok();
    info_now!("entering init_board");
    let hal = hal::Peripherals::from((device_peripherals, core_peripherals));
    
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
        })
    };

    let mut initializer = initializer::Initializer::new(config, hal.syscon, hal.pmc, hal.anactrl);

    let mut final_stage = initializer.initialize_all(
        hal.iocon,
        hal.gpio,
        hal.adc,
        hal.dma,
        hal.ctimer.0,
        hal.ctimer.1,
        hal.ctimer.2,
        hal.ctimer.3,
        hal.ctimer.4,
        hal.pfr,
        hal.flexcomm.0,
        hal.inputmux,
        hal.pint,
        hal.usbhs,
        hal.usbfs,
        hal.rng,
        hal.prince,
        hal.flash,


        hal.rtc,
    );

    let _is_passive_mode = initializer.is_in_passive_operation(&final_stage);
    let clock_controller = initializer.get_dynamic_clock_control(&mut final_stage);

    // rgb.turn_off();
    info!("init took {} ms", final_stage.perf_timer.elapsed().0/1000);

    #[cfg(feature = "provisioner-app")]
    let store = final_stage.store.clone();
    #[cfg(feature = "provisioner-app")]
    let internal_fs = final_stage.filesystem_stage.internal_storage_fs;

    let apps = types::Apps::new(
        &mut final_stage.trussed,
        #[cfg(feature = "provisioner-app")]
        {
            types::ProvisionerNonPortable {
                store,
                stolen_filesystem: internal_fs.as_mut().unwrap(),
                nfc_powered: _is_passive_mode,
            }
        }
    );

    (
        final_stage.filesystem_stage.flash_stage.interfaces_stage.apdu_dispatch,
        final_stage.filesystem_stage.flash_stage.interfaces_stage.ctaphid_dispatch,
        final_stage.trussed,

        apps,

        final_stage.filesystem_stage.flash_stage.interfaces_stage.usb_stage.usb_classes,
        final_stage.filesystem_stage.flash_stage.interfaces_stage.usb_stage.nfc_stage.iso14443,

        final_stage.filesystem_stage.flash_stage.interfaces_stage.usb_stage.nfc_stage.basic_stage.perf_timer,
        clock_controller,

        final_stage.filesystem_stage.flash_stage.interfaces_stage.usb_stage.nfc_stage.basic_stage.delay_timer,
    )
}
