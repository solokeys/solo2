#![no_std]

// panic handler, depending on debug/release build
// BUT: need to run in release anyway, to have USB work
// #[cfg(debug_assertions)]
use panic_semihosting as _;
// #[cfg(not(debug_assertions))]
// use panic_halt as _;

use core::convert::TryInto;

// board support package
#[cfg(not(any(feature = "board-lpcxpresso", feature = "board-prototype")))]
compile_error!("Please select one of the board support packages.");

#[cfg(feature = "board-lpcxpresso")]
pub use lpcxpresso55 as board;

#[cfg(feature = "board-prototype")]
pub use prototype_bee as board;

logging::add!(logger);

use c_stubs as _;

// re-exports for convenience
pub use board::hal;
pub use board::rt::entry;

pub mod types;
pub mod clock_controller;
pub mod wink;
pub mod solo_trussed;
use types::{
    Board,
    EnabledUsbPeripheral,
    ExternalStorage,
    VolatileStorage,
    Store,
};

use fm11nc08::{
    FM11NC08, Configuration, Register,
};
use hal::drivers::timer::Lap;
use hal::traits::wg::timer::Cancel;

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

use interchange::Interchange;
use usbd_ccid::Ccid;
use usbd_ctaphid::CtapHid;
// use usbd_ctaphid::insecure::InsecureRamAuthenticator;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
// bring traits in scope
use hal::prelude::*;
use hal::traits::wg::digital::v2::InputPin;


fn configure_fm11_if_needed(
    fm: &mut types::NfcChip,
    timer: &mut Timer<impl hal::peripherals::ctimer::Ctimer<hal::typestates::init_state::Enabled>>)
    -> Result<(),()>
    {
    //                      no limit      2mA resistor    3.3V
    const REGU_CONFIG: u8 = (0b11 << 4) | (0b10 << 2) | (0b11 << 0);
    let current_regu_config = fm.read_reg(fm11nc08::Register::ReguCfg);

    if current_regu_config == 0xff {
        // No nfc chip connected
        logger::info!("No NFC chip connected").ok();
        return Err(());
    }

    if  current_regu_config != REGU_CONFIG {
    // if true {
        logger::info!("{}", fm.dump_eeprom() ).ok();
        logger::info!("{}", fm.dump_registers() ).ok();

        logger::info!("writing EEPROM").ok();

        fm.configure(Configuration{
            regu: REGU_CONFIG,
            ataq: 0x4400,
            sak1: 0x04,
            sak2: 0x20,
            tl: 0x05,
            // (x[7:4], FSDI[3:0]) . FSDI[2] == 32 byte frame, FSDI[8] == 256 byte frame, 7==128byte
            t0: 0x78,
            ta: 0x91,
            // (FWI[b4], SFGI[b4]), (256 * 16 / fc) * 2 ^ value
            tb: 0x78,
            tc: 0x02,
                // enable P-on IRQ    14443-4 mode
            nfc:    (0b0 << 1) |       (0b00 << 2),
        }, timer);
    } else {
        logger::info!("EEPROM already initialized.").ok();
    }

    // disable all interrupts except RxStart
    fm.write_reg(Register::AuxIrqMask, 0x00);
    fm.write_reg(Register::FifoIrqMask,
        // 0x0
        0xff
        ^ (1 << 3) /* water-level */
        ^ (1 << 1) /* fifo-full */
    );
    fm.write_reg(Register::MainIrqMask,
        // 0x0
        0xff
        ^ fm11nc08::device::Interrupt::RxStart as u8
        ^ fm11nc08::device::Interrupt::RxDone as u8
        ^ fm11nc08::device::Interrupt::TxDone as u8
        ^ fm11nc08::device::Interrupt::Fifo as u8
        ^ fm11nc08::device::Interrupt::Active as u8
    );

    //                    no limit    rrfcfg .      3.3V
    // let regu_powered = (0b11 << 4) | (0b10 << 2) | (0b11 << 0);
    // fm.write_reg(Register::ReguCfg, regu_powered);

    Ok(())
}

// // filesystem starting at 320KB
// // this needs to be synchronized with contents of `memory.x`
// const FS_BASE: usize = 0x50_000;

// TODO: move board-specifics to BSPs
// #[cfg(feature = "board-lpcxpresso")]
pub fn init_board(device_peripherals: hal::raw::Peripherals, core_peripherals: rtic::Peripherals) -> (
    // types::Authenticator,
    types::ApduDispatch,
    types::HidDispatch,
    types::CryptoService,

    types::Piv,
    types::FidoApplet<fido_authenticator::NonSilentAuthenticator>,
    applet_ndef::NdefApplet<'static>,

    Option<types::UsbClasses>,
    Option<types::Iso14443>,

    types::PerfTimer,
    Option<clock_controller::DynamicClockController>,
    types::HwScheduler,
) {
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
            .system_frequency(4.mhz())
            .configure(&mut anactrl, &mut pmc, &mut syscon)
            .expect("Clock configuration failed");

        // important to start Adc early in passive mode
        let adc = hal::Adc::from(hal.adc)
            .configure(clock_controller::DynamicClockController::adc_configuration())
            .enabled(&mut pmc, &mut syscon);
        (clocks, adc)
    } else {
        let clocks = hal::ClockRequirements::default()
            .system_frequency(96.mhz())
            .configure(&mut anactrl, &mut pmc, &mut syscon)
            .expect("Clock configuration failed");

        let adc = hal::Adc::from(hal.adc)
            .enabled(&mut pmc, &mut syscon);

        (clocks, adc)
    };

    let mut delay_timer = Timer::new(hal.ctimer.0.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap()));
    let mut perf_timer = Timer::new(hal.ctimer.4.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap()));
    perf_timer.start(60_000.ms());

    #[cfg(feature = "board-lpcxpresso")]
    let rgb = board::led::RgbLed::new(
        board::led::RedLedPin::take().unwrap(),
        board::led::GreenLedPin::take().unwrap(),
        board::led::BlueLedPin::take().unwrap(),
        Pwm::new(hal.ctimer.2.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap())),
        &mut iocon,
    );

    #[cfg(feature = "board-prototype")]
    let rgb = board::led::RgbLed::new(
        board::led::RedLedPin::take().unwrap(),
        board::led::GreenLedPin::take().unwrap(),
        board::led::BlueLedPin::take().unwrap(),
        Pwm::new(hal.ctimer.3.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap())),
        &mut iocon,
    );

    if is_passive_mode {
        // Give a small delay to charge up capacitors
        delay_timer.start(4.ms()); nb::block!(delay_timer.wait()).ok();
    }

    let usb0_vbus_pin = pins::Pio0_22::take().unwrap()
        .into_usb0_vbus_pin(&mut iocon);

    let rng = hal.rng.enabled(&mut syscon);

    use littlefs2::fs::{Allocation, Filesystem};

    let flash = hal::drivers::flash::FlashGordon::new(hal.flash.enabled(&mut syscon));

    static mut INTERNAL_STORAGE: Option<FlashGordon> = None;
    unsafe { INTERNAL_STORAGE = Some(flash); }
    static mut INTERNAL_FS_ALLOC: Option<Allocation<FlashGordon>> = None;
    unsafe { INTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }

    static mut EXTERNAL_STORAGE: ExternalStorage = ExternalStorage::new();
    static mut EXTERNAL_FS_ALLOC: Option<Allocation<ExternalStorage>> = None;
    unsafe { EXTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }

    static mut VOLATILE_STORAGE: VolatileStorage = VolatileStorage::new();
    static mut VOLATILE_FS_ALLOC: Option<Allocation<VolatileStorage>> = None;
    unsafe { VOLATILE_FS_ALLOC = Some(Filesystem::allocate()); }

    // temporarily increase clock for the storage mounting or else it takes a long time.
    if is_passive_mode {
        clocks = unsafe { hal::ClockRequirements::default()
            .system_frequency(48.mhz())
            .reconfigure(clocks, &mut pmc, &mut syscon) };
    }
    let store = Store::claim().unwrap();


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

    if result.is_err() {
        logger::info!("Not yet formatted!  Formatting..").ok();
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

    // return to slow freq
    if is_passive_mode {
        clocks = unsafe { hal::ClockRequirements::default()
            .system_frequency(12.mhz())
            .reconfigure(clocks, &mut pmc, &mut syscon) };
        // // Give some feedback to user that token is in field
        // rgb.red(30);
    }

    let (fido_trussed_requester, fido_trussed_responder) = trussed::pipe::TrussedInterchange::claim(0)
        .expect("could not setup FIDO TrussedInterchange");
    let mut fido_client_id = littlefs2::path::PathBuf::new();
    fido_client_id.push(b"fido2\0".try_into().unwrap());

    let (contact_requester, contact_responder) = usbd_ccid::types::ApduInterchange::claim(0)
        .expect("could not setup ccid ApduInterchange");

    let (contactless_requester, contactless_responder) = iso14443::types::ApduInterchange::claim(0)
        .expect("could not setup iso14443 ApduInterchange");

    let (hid_requester, hid_responder) = hid_dispatch::types::HidInterchange::claim(0)
        .expect("could not setup HidInterchange");

    let (piv_trussed_requester, piv_trussed_responder) = trussed::pipe::TrussedInterchange::claim(1)
        .expect("could not setup PIV TrussedInterchange");

    let usb_classes =
    {
        let mut usbd = hal.usbhs.enabled_as_device(
            &mut anactrl,
            &mut pmc,
            &mut syscon,
            &mut delay_timer,
            clocks.support_usbhs_token().unwrap(),
        );
        #[cfg(not(feature = "highspeed"))]
        usbd.disable_high_speed();
        let _: EnabledUsbPeripheral = usbd;

        // ugh, what's the nice way?
        static mut USB_BUS: Option<usb_device::bus::UsbBusAllocator<UsbBus<EnabledUsbPeripheral>>> = None;
        unsafe { USB_BUS = Some(hal::drivers::UsbBus::new(usbd, usb0_vbus_pin)); }
        let usb_bus = unsafe { USB_BUS.as_ref().unwrap() };

        // our USB classes (must be allocated in order that they're passed in `.poll(...)` later!)
        let ccid = Ccid::new(usb_bus, contact_requester);
        let ctaphid = CtapHid::new(usb_bus, hid_requester, perf_timer.lap().0/1000)
                        .implements_ctap1()
                        .implements_ctap2()
                        .implements_wink();
        let serial = usbd_serial::SerialPort::new(usb_bus);

        // our composite USB device
        let usbd = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x1209, 0xbeee))
            .manufacturer("SoloKeys")
            .product("Solo 🐝")
            .serial_number("20/20")
            .device_release(0x0001)
            .max_packet_size_0(64)
            .build();

        Some(types::UsbClasses::new(usbd, ccid, ctaphid, serial))
    };

    let iso14443 = {
        let token = clocks.support_flexcomm_token().unwrap();
        let spi = hal.flexcomm.0.enabled_as_spi(&mut syscon, &token);
        let sck = types::NfcSckPin::take().unwrap().into_spi0_sck_pin(&mut iocon);
        let mosi = types::NfcMosiPin::take().unwrap().into_spi0_mosi_pin(&mut iocon);
        let miso = types::NfcMisoPin::take().unwrap().into_spi0_miso_pin(&mut iocon);
        let spi_mode = hal::traits::wg::spi::Mode {
            polarity: hal::traits::wg::spi::Polarity::IdleLow,
            phase: hal::traits::wg::spi::Phase::CaptureOnSecondTransition,
        };
        let spi = SpiMaster::new(spi, (sck, mosi, miso, hal::typestates::pin::flexcomm::NoCs), 2.mhz(), spi_mode);

        // Start unselected.
        let nfc_cs = types::NfcCsPin::take().unwrap().into_gpio_pin(&mut iocon, &mut gpio).into_output_high();


        // Set up external interrupt for NFC IRQ
        let mut mux = hal.inputmux.enabled(&mut syscon);
        let mut pint = hal.pint.enabled(&mut syscon);
        pint.enable_interrupt(&mut mux, &nfc_irq, hal::peripherals::pint::Slot::Slot0, hal::peripherals::pint::Mode::ActiveLow);
        mux.disabled(&mut syscon);

        let mut fm = FM11NC08::new(spi, nfc_cs, nfc_irq).enabled();
        if configure_fm11_if_needed(&mut fm, &mut delay_timer).is_ok() {
            Some(iso14443::Iso14443::new(fm, contactless_requester))
        } else {
            if is_passive_mode {
                logger::info!("Shouldn't get passive signal when there's no chip!").ok();
            }
            None
        }
    };

    let mut rtc = hal.rtc.enabled(&mut syscon, clocks.enable_32k_fro(&mut pmc));
    rtc.reset();

    let (clock_controller, three_buttons) = if is_passive_mode {
        let signal_pin = types::SignalPin::take().unwrap().into_gpio_pin(&mut iocon, &mut gpio).into_output_low();
        let mut clock_controller = clock_controller::DynamicClockController::new(adc, signal_pin, clocks, pmc, syscon);
        clock_controller.start_high_voltage_compare();
        (Some(clock_controller), None)
    } else {
        #[cfg(feature = "board-lpcxpresso")]
        let three_buttons = board::button::ThreeButtons::new(
            Timer::new(hal.ctimer.1.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap())),
            board::button::UserButtonPin::take().unwrap().into_gpio_pin(&mut iocon, &mut gpio).into_input(),
            board::button::WakeupButtonPin::take().unwrap().into_gpio_pin(&mut iocon, &mut gpio).into_input(),
        );

        #[cfg(feature = "board-prototype")]
        let three_buttons =
        {
            let mut dma = hal::Dma::from(hal.dma).enabled(&mut syscon);

            board::button::ThreeButtons::new (
                adc,
                hal.ctimer.1.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap()),
                hal.ctimer.2.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap()),
                board::button::ChargeMatchPin::take().unwrap().into_match_output(&mut iocon),
                board::button::ButtonTopPin::take().unwrap().into_analog_input(&mut iocon, &mut gpio),
                board::button::ButtonBotPin::take().unwrap().into_analog_input(&mut iocon, &mut gpio),
                board::button::ButtonMidPin::take().unwrap().into_analog_input(&mut iocon, &mut gpio),
                &mut dma,
                clocks.support_touch_token().unwrap(),
            )
        };
        (None, Some(three_buttons))
    };

    let rgb = if is_passive_mode {
        None
    } else {
        Some(rgb)
    };

    let solobee_interface = solo_trussed::UserInterface::new(three_buttons, rgb);
    let solobee_uptime = solo_trussed::UpTime::new(rtc);

    let board = Board::new(rng, store, solobee_uptime, solobee_interface);
    let mut trussed = trussed::service::Service::new(board);

    let mut piv_client_id = littlefs2::path::PathBuf::new();
    piv_client_id.push(b"piv2\0".try_into().unwrap());
    assert!(trussed.add_endpoint(piv_trussed_responder, piv_client_id).is_ok());

    let syscaller = trussed::client::TrussedSyscall::default();
    let piv_trussed = trussed::client::Client::new(
        piv_trussed_requester,
        syscaller,
    );

    assert!(trussed.add_endpoint(fido_trussed_responder, fido_client_id).is_ok());

    let syscaller = trussed::client::TrussedSyscall::default();
    let trussed_client = trussed::client::Client::new(fido_trussed_requester, syscaller);

    let authnr = fido_authenticator::Authenticator::new(
        trussed_client,
        fido_authenticator::NonSilentAuthenticator {},
    );

    let fido = applet_fido::Fido::new(authnr);

    let piv = piv_card::App::new(piv_trussed);
    let ndef = applet_ndef::NdefApplet::new();

    let apdu_dispatch = types::ApduDispatch::new(contact_responder, contactless_responder);
    let hid_dispatch = types::HidDispatch::new(hid_responder);

    // rgb.turn_off();
    delay_timer.cancel().ok();
    logger::info!("init took {} ms",perf_timer.lap().0/1000).ok();

    (
        apdu_dispatch,
        hid_dispatch,
        trussed,

        piv,
        fido,
        ndef,

        usb_classes,
        iso14443,

        perf_timer,
        clock_controller,
        delay_timer,
    )
}

//
// Logging
//
use logging::{funnel,Drain};
use rtic::Mutex;
funnel!(NVIC_PRIO_BITS = hal::raw::NVIC_PRIO_BITS, {
    0: 2048,
    1: 1024,
    2: 1024,
    3: 8192,
    4: 1024,
});

pub fn drain_log_to_serial(mut serial: impl Mutex<T = types::SerialClass>) {
    let mut buf = [0u8; 64];

    let drains = Drain::get_all();

    for (_, drain) in drains.iter().enumerate() {
        'l: loop {
            let n = drain.read(&mut buf).len();
            if n == 0 {
                break 'l;
            }
            serial.lock(|serial: &mut types::SerialClass| {
                match serial.write(&buf[..n]) {
                    Ok(_count) => {
                    },
                    Err(_err) => {
                    },
                }

                // not much we can do
                serial.flush().ok();
            });
        }
    }
}

pub fn drain_log_to_semihosting() {
    let drains = Drain::get_all();
    let mut buf = [0u8; 64];

    for (_, drain) in drains.iter().enumerate() {
        'l: loop {
            let n = drain.read(&mut buf).len();
            if n == 0 {
                break 'l;
            }
            match core::str::from_utf8(&buf[..n]) {
                Ok(string) => logging::write!(string).ok(),
                Err(e) => logging::blocking::error!("ERROR {:?}", &e).ok(),
            };
        }
    }
}

