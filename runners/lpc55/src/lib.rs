#![no_std]
include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

// panic handler, depending on debug/release build
// BUT: need to run in release anyway, to have USB work
use panic_halt as _;
// use panic_semihosting as _;

#[macro_use]
extern crate delog;
generate_macros!();

use core::convert::TryInto;

use board::clock_controller;

use c_stubs as _;

// re-exports for convenience
pub use board::hal;
// pub use board::rt::entry;

pub mod types;

use types::{
    Board,
    EnabledUsbPeripheral,
    ExternalStorage,
    VolatileStorage,
    Store,
};

use hal::drivers::timer::Elapsed;
use hal::traits::wg::timer::Cancel;
use trussed::platform::UserInterface;

//
// Board Initialization
//

use hal::drivers::{
    flash::FlashGordon,
    pins,
    UsbBus,
    Timer,
    Pwm,
};
use hal::peripherals::pfr::Pfr;
use board::traits::rgb_led::RgbLed;
use board::traits::buttons::Press;
use interchange::Interchange;
use usbd_ccid::Ccid;
use usbd_ctaphid::CtapHid;
// use usbd_ctaphid::insecure::InsecureRamAuthenticator;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
// bring traits in scope
use hal::prelude::*;
use hal::traits::wg::digital::v2::InputPin;

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

fn validate_cfpa(pfr: &mut Pfr<hal::typestates::init_state::Enabled>) {
    let mut cfpa = pfr.read_latest_cfpa().unwrap();
    let current_version: u32 = build_constants::CARGO_PKG_VERSION;
    if cfpa.secure_fw_version < current_version || cfpa.ns_fw_version < current_version {
        info!("updating cfpa from {} to {}", cfpa.secure_fw_version, current_version);

        // All of these are monotonic counters.
        cfpa.version += 1;
        cfpa.secure_fw_version = current_version;
        cfpa.ns_fw_version = current_version;
        pfr.write_cfpa(&cfpa).unwrap();
    } else {
        info!("do not need to update cfpa version {}", cfpa.secure_fw_version);
    }
        // Unless encryption is explicity disabled, we require that PRINCE has been provisioned.
    #[cfg(not(feature = "no-encrypted-storage"))]
    assert!(
        cfpa.key_provisioned(hal::peripherals::pfr::KeyType::PrinceRegion2)
    );
}

fn get_serial_number() -> &'static str {
    static mut SERIAL_NUMBER: heapless::String<heapless::consts::U36> = heapless::String(heapless::i::String::new());
    use core::fmt::Write;
    unsafe {
        let uuid = crate::hal::uuid();
        SERIAL_NUMBER.write_fmt(format_args!("{}", hexstr!(&uuid))).unwrap();
        &SERIAL_NUMBER
    }
}

// SoloKeys stores a product string in the first 64 bytes of CMPA.
fn get_product_string(pfr: &mut Pfr<hal::typestates::init_state::Enabled>) -> &'static str {
    let data = pfr.cmpa_customer_data();

    // check the first 64 bytes of customer data for a string
    if data[0] != 0 {
        for i in 1 .. 64 {
            if data[i] == 0 {
                let str_maybe = core::str::from_utf8(&data[0 .. i]);
                if let Ok(string) = str_maybe {
                    return string;
                }
                break;
            }
        }
    }

    // Use a default string
    "Solo 2 (custom)"
}

// TODO: move board-specifics to BSPs
pub fn init_board(
    device_peripherals: hal::raw::Peripherals,
    core_peripherals: rtic::Peripherals,
) -> (
    // types::Authenticator,
    types::ApduDispatch,
    types::CtaphidDispach,
    types::Trussed,

    types::Piv,
    types::Totp,
    types::FidoApp<fido_authenticator::NonSilentAuthenticator>,
    ndef_app::App<'static>,
    types::ManagementApp,

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

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;

    let mut gpio = hal.gpio.enabled(&mut syscon);
    let mut iocon = hal.iocon.enabled(&mut syscon);

    let nfc_irq = types::NfcIrqPin::take().unwrap().into_gpio_pin(&mut iocon, &mut gpio).into_input();
    // Need to enable pullup for NFC IRQ input.
    let iocon = iocon.release();
    iocon.pio0_19.modify(|_,w| { w.mode().pull_up() } );
    let mut iocon = hal::Iocon::from(iocon).enabled(&mut syscon);
    let is_passive_mode = nfc_irq.is_low().ok().unwrap();

    // Start out with slow clock if in passive mode;
    let (mut clocks, adc) = if is_passive_mode {
        let clocks = hal::ClockRequirements::default()
            .system_frequency(4.MHz())
            .configure(&mut anactrl, &mut pmc, &mut syscon)
            .expect("Clock configuration failed");

        // important to start Adc early in passive mode
        let adc = hal::Adc::from(hal.adc)
            .configure(clock_controller::DynamicClockController::adc_configuration())
            .enabled(&mut pmc, &mut syscon);
        (clocks, adc)
    } else {
        let clocks = hal::ClockRequirements::default()
            .system_frequency(96.MHz())
            .configure(&mut anactrl, &mut pmc, &mut syscon)
            .expect("Clock configuration failed");

        let adc = hal::Adc::from(hal.adc)
            .enabled(&mut pmc, &mut syscon);


        (clocks, adc)
    };

    let mut delay_timer = Timer::new(hal.ctimer.0.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap()));
    let mut perf_timer = Timer::new(hal.ctimer.4.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap()));
    perf_timer.start(60_000_000.microseconds());

    #[cfg(feature = "board-lpcxpresso55")]
    let mut rgb = board::RgbLed::new(
        Pwm::new(hal.ctimer.2.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap())),
        &mut iocon,
    );

    #[cfg(feature = "board-solo2")]
    let mut rgb = board::RgbLed::new(
        Pwm::new(hal.ctimer.3.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap())),
        &mut iocon,
    );

    let (three_buttons,adc) = if is_passive_mode {
        (None, Some(adc))
    } else {
        #[cfg(feature = "board-lpcxpresso55")]
        let three_buttons = board::ThreeButtons::new(
            Timer::new(hal.ctimer.1.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap())),
            &mut gpio,
            &mut iocon,
        );

        #[cfg(feature = "board-solo2")]
        let three_buttons =
        {
            let mut dma = hal::Dma::from(hal.dma).enabled(&mut syscon);

            board::ThreeButtons::new (
                adc,
                hal.ctimer.1.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap()),
                hal.ctimer.2.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap()),
                &mut dma,
                clocks.support_touch_token().unwrap(),
                &mut gpio,
                &mut iocon,
            )
        };

        // Boot to bootrom if buttons are all held for 5s
        info!("button start {}",perf_timer.elapsed().0/1000);
        delay_timer.start(5_000_000.microseconds());
        while three_buttons.is_pressed(board::traits::buttons::Button::A) &&
              three_buttons.is_pressed(board::traits::buttons::Button::B) &&
              three_buttons.is_pressed(board::traits::buttons::Button::Middle) {
            // info!("3 buttons pressed..");
            if delay_timer.wait().is_ok() {
                // Give a small red blink show success
                rgb.red(200);
                rgb.green(0);
                rgb.blue(0);
                delay_timer.start(100_000.microseconds()); nb::block!(delay_timer.wait()).ok();
                board::hal::boot_to_bootrom()
            }
        }
        delay_timer.cancel().ok();

        info!("button end {}",perf_timer.elapsed().0/1000);
        (Some(three_buttons), None)
    };

    let mut pfr = hal.pfr.enabled(&clocks).unwrap();
    validate_cfpa(&mut pfr);


    let (contactless_requester, contactless_responder) = apdu_dispatch::interchanges::Contactless::claim()
        .expect("could not setup iso14443 ApduInterchange");
    let mut iso14443 = {
        let token = clocks.support_flexcomm_token().unwrap();
        let spi = hal.flexcomm.0.enabled_as_spi(&mut syscon, &token);

        // Set up external interrupt for NFC IRQ
        let mut mux = hal.inputmux.enabled(&mut syscon);
        let mut pint = hal.pint.enabled(&mut syscon);
        pint.enable_interrupt(&mut mux, &nfc_irq, hal::peripherals::pint::Slot::Slot0, hal::peripherals::pint::Mode::ActiveLow);
        mux.disabled(&mut syscon);

        let force_nfc_reconfig = cfg!(feature = "reconfigure-nfc");

        let maybe_fm = board::nfc::try_setup(
            spi,
            &mut gpio,
            &mut iocon,
            nfc_irq,
            &mut delay_timer,
            force_nfc_reconfig,
        );

        if let Ok(fm) = maybe_fm {
            Some(nfc_device::Iso14443::new(fm, contactless_requester))
        } else {
            if is_passive_mode {
                info!("Shouldn't get passive signal when there's no chip!");
            }
            None
        }
    };

    if let Some(iso14443) = &mut iso14443 { iso14443.poll(); }
    if is_passive_mode {
        // Give a small delay to charge up capacitors
        delay_timer.start(5_000.microseconds()); nb::block!(delay_timer.wait()).ok();
    }
    if let Some(iso14443) = &mut iso14443 { iso14443.poll(); }

    let usb0_vbus_pin = pins::Pio0_22::take().unwrap()
        .into_usb0_vbus_pin(&mut iocon);

    #[allow(unused_mut)]
    let mut rng = hal.rng.enabled(&mut syscon);

    let prince = hal.prince.enabled(&mut rng);
    prince.disable_all_region_2();

    use littlefs2::fs::{Allocation, Filesystem};

    let flash_gordon = FlashGordon::new(hal.flash.enabled(&mut syscon));

    #[cfg(not(feature = "no-encrypted-storage"))]
    let filesystem = types::PrinceFilesystem::new(flash_gordon, prince);

    #[cfg(feature = "no-encrypted-storage")]
    let filesystem = types::PlainFilesystem::new(flash_gordon);

    // temporarily increase clock for the storage mounting or else it takes a long time.
    if is_passive_mode {
        clocks = unsafe { hal::ClockRequirements::default()
            .system_frequency(48.MHz())
            .reconfigure(clocks, &mut pmc, &mut syscon) };
    }
    info!("mount start {} ms",perf_timer.elapsed().0/1000);
    static mut INTERNAL_STORAGE: Option<types::FlashStorage> = None;
    unsafe { INTERNAL_STORAGE.replace(filesystem); }
    static mut INTERNAL_FS_ALLOC: Option<Allocation<types::FlashStorage>> = None;
    unsafe { INTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }

    static mut EXTERNAL_STORAGE: ExternalStorage = ExternalStorage::new();
    static mut EXTERNAL_FS_ALLOC: Option<Allocation<ExternalStorage>> = None;
    unsafe { EXTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }

    static mut VOLATILE_STORAGE: VolatileStorage = VolatileStorage::new();
    static mut VOLATILE_FS_ALLOC: Option<Allocation<VolatileStorage>> = None;
    unsafe { VOLATILE_FS_ALLOC = Some(Filesystem::allocate()); }


    let store = Store::claim().unwrap();

    if let Some(iso14443) = &mut iso14443 { iso14443.poll(); }

    let result = store.mount(
        unsafe { INTERNAL_FS_ALLOC.as_mut().unwrap() },
        // unsafe { &mut INTERNAL_STORAGE },
        unsafe { INTERNAL_STORAGE.as_mut().unwrap() },
        unsafe { EXTERNAL_FS_ALLOC.as_mut().unwrap() },
        unsafe { &mut EXTERNAL_STORAGE },
        unsafe { VOLATILE_FS_ALLOC.as_mut().unwrap() },
        unsafe { &mut VOLATILE_STORAGE },
        // to trash existing data, set to true
        false,
    );


    if result.is_err() || cfg!(feature = "format-filesystem") {
        rgb.blue(200);
        rgb.red(200);
        delay_timer.start(300_000.microseconds()); nb::block!(delay_timer.wait()).ok();
        info!("Not yet formatted!  Formatting..");
        store.mount(
            unsafe { INTERNAL_FS_ALLOC.as_mut().unwrap() },
            // unsafe { &mut INTERNAL_STORAGE },
            unsafe { INTERNAL_STORAGE.as_mut().unwrap() },
            unsafe { EXTERNAL_FS_ALLOC.as_mut().unwrap() },
            unsafe { &mut EXTERNAL_STORAGE },
            unsafe { VOLATILE_FS_ALLOC.as_mut().unwrap() },
            unsafe { &mut VOLATILE_STORAGE },
            // to trash existing data, set to true
            true,
        ).unwrap();
    }
    info!("mount end {} ms",perf_timer.elapsed().0/1000);

    // return to slow freq
    if is_passive_mode {
        clocks = unsafe { hal::ClockRequirements::default()
            .system_frequency(12.MHz())
            .reconfigure(clocks, &mut pmc, &mut syscon) };
        // // Give some feedback to user that token is in field
        // rgb.red(30);
    }

    if let Some(iso14443) = &mut iso14443 { iso14443.poll(); }

    let (fido_trussed_requester, fido_trussed_responder) = trussed::pipe::TrussedInterchange::claim()
        .expect("could not setup FIDO TrussedInterchange");
    let mut fido_client_id = littlefs2::path::PathBuf::new();
    fido_client_id.push(b"fido2\0".try_into().unwrap());

    let (management_trussed_requester, management_trussed_responder) = trussed::pipe::TrussedInterchange::claim()
        .expect("could not setup FIDO TrussedInterchange");
    let mut management_client_id = littlefs2::path::PathBuf::new();
    management_client_id.push(b"management\0".try_into().unwrap());

    let (contact_requester, contact_responder) = apdu_dispatch::interchanges::Contact::claim()
        .expect("could not setup ccid ApduInterchange");

    let (hid_requester, hid_responder) = ctaphid_dispatch::types::HidInterchange::claim()
        .expect("could not setup HidInterchange");

    let (piv_trussed_requester, piv_trussed_responder) = trussed::pipe::TrussedInterchange::claim()
        .expect("could not setup PIV TrussedInterchange");

    let (totp_trussed_requester, totp_trussed_responder) = trussed::pipe::TrussedInterchange::claim()
        .expect("could not setup TOTP TrussedInterchange");

    info!("usb class start {} ms",perf_timer.elapsed().0/1000);
    let usb_classes =
    {
        if !is_passive_mode {
            #[cfg(not(feature = "usbfs-peripheral"))]
            let mut usbd = hal.usbhs.enabled_as_device(
                &mut anactrl,
                &mut pmc,
                &mut syscon,
                &mut delay_timer,
                clocks.support_usbhs_token().unwrap(),
            );
            #[cfg(feature = "usbfs-peripheral")]
            let usbd = hal.usbfs.enabled_as_device(
                &mut anactrl,
                &mut pmc,
                &mut syscon,
                clocks.support_usbfs_token().unwrap(),
            );
            #[cfg(not(any(feature = "highspeed", feature = "usbfs-peripheral")))]
            usbd.disable_high_speed();
            let _: EnabledUsbPeripheral = usbd;

            // ugh, what's the nice way?
            static mut USB_BUS: Option<usb_device::bus::UsbBusAllocator<UsbBus<EnabledUsbPeripheral>>> = None;
            unsafe { USB_BUS.replace(hal::drivers::UsbBus::new(usbd, usb0_vbus_pin)); }
            let usb_bus = unsafe { USB_BUS.as_ref().unwrap() };

            // our USB classes (must be allocated in order that they're passed in `.poll(...)` later!)
            let ccid = Ccid::new(usb_bus, contact_requester);
            let ctaphid = CtapHid::new(usb_bus, hid_requester, perf_timer.elapsed().0/1000)
                .implements_ctap1()
                .implements_ctap2()
                .implements_wink();

            let serial = usbd_serial::SerialPort::new(usb_bus);

            // Only 16 bits, so take the upper bits of our semver
            let device_release =
                ((build_constants::CARGO_PKG_VERSION_MAJOR as u16) << 8) |
                (build_constants::CARGO_PKG_VERSION_MINOR as u16);

            // our composite USB device
            let product_string = get_product_string(&mut pfr);
            let serial_number = get_serial_number();

            let usbd = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x1209, 0xbeee))
                .manufacturer("SoloKeys")
                .product(product_string)
                .serial_number(serial_number)
                .device_release(device_release)
                .max_packet_size_0(64)
                .composite_with_iads()
                .build();

            Some(types::UsbClasses::new(usbd, ccid, ctaphid, /*keyboard,*/ serial))
        } else {
            None
        }
    };

    let mut rtc = hal.rtc.enabled(&mut syscon, clocks.enable_32k_fro(&mut pmc));
    rtc.reset();

    let rgb = if is_passive_mode {
        None
    } else {
        Some(rgb)
    };

    let clock_controller = if is_passive_mode {
        let mut clock_controller = clock_controller::DynamicClockController::new(adc.unwrap(),
            clocks, pmc, syscon, &mut gpio, &mut iocon);
        clock_controller.start_high_voltage_compare();
        Some(clock_controller)
    } else {
        None
    };

    let mut solobee_interface = board::trussed::UserInterface::new(rtc, three_buttons, rgb);
    solobee_interface.set_status(trussed::platform::ui::Status::Idle);

    let board = Board::new(rng, store, solobee_interface);
    let mut trussed = trussed::service::Service::new(board);

    let mut piv_client_id = littlefs2::path::PathBuf::new();
    piv_client_id.push(b"piv\0".try_into().unwrap());
    assert!(trussed.add_endpoint(piv_trussed_responder, piv_client_id).is_ok());

    let mut totp_client_id = littlefs2::path::PathBuf::new();
    totp_client_id.push(b"totp\0".try_into().unwrap());
    assert!(trussed.add_endpoint(totp_trussed_responder, totp_client_id).is_ok());

    let syscaller = types::Syscall::default();
    let piv_trussed = types::TrussedClient::new(
        piv_trussed_requester,
        syscaller,
    );

    let syscaller = types::Syscall::default();
    let totp_trussed = types::TrussedClient::new(
        totp_trussed_requester,
        syscaller,
    );

    let syscaller = types::Syscall::default();
    let management_trussed = types::TrussedClient::new(management_trussed_requester, syscaller);

    let syscaller = types::Syscall::default();
    let trussed_client = types::TrussedClient::new(fido_trussed_requester, syscaller);

    assert!(trussed.add_endpoint(fido_trussed_responder, fido_client_id).is_ok());
    assert!(trussed.add_endpoint(management_trussed_responder, management_client_id).is_ok());

    let authnr = fido_authenticator::Authenticator::new(
        trussed_client,
        fido_authenticator::NonSilentAuthenticator {},
    );

    let fido = dispatch_fido::Fido::new(authnr);

    let piv = piv_authenticator::Authenticator::new(piv_trussed);
    let ndef = ndef_app::App::new();
    let management = types::ManagementApp::new(management_trussed, hal::uuid(), build_constants::CARGO_PKG_VERSION);
    let totp = oath_authenticator::Authenticator::new(totp_trussed);

    let apdu_dispatch = types::ApduDispatch::new(contact_responder, contactless_responder);
    let ctaphid_dispatch = types::CtaphidDispach::new(hid_responder);

    // rgb.turn_off();
    delay_timer.cancel().ok();
    info!("init took {} ms",perf_timer.elapsed().0/1000);

    (
        apdu_dispatch,
        ctaphid_dispatch,
        trussed,

        piv,
        totp,
        fido,
        ndef,
        management,

        usb_classes,
        iso14443,

        perf_timer,
        clock_controller,
        delay_timer,
    )
}
