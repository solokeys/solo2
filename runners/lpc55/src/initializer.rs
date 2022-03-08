use crate::hal;
use hal::prelude::*;
use hal::traits::wg::digital::v2::InputPin;
use hal::traits::wg::timer::Cancel;
use hal::drivers::timer::Elapsed;
use hal::drivers::{
    clocks::Clocks,
    flash::FlashGordon,
    pins::direction,
    pins,
    UsbBus,
    Pwm,
    Timer
};
use hal::typestates::pin::state::Gpio;
use hal::peripherals::{
    ctimer,
    ctimer::Ctimer,
};
use hal::peripherals::pfr::Pfr;
use hal::typestates::init_state::Unknown;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};

use interchange::Interchange;
use trussed::platform::UserInterface;

use board::traits::buttons;
use board::traits::buttons::Press;
use board::traits::rgb_led::RgbLed;

use crate::{types, clock_controller, build_constants};

pub mod stages;

pub trait State {}
pub struct Booted;
pub struct EnabledIo;

pub enum UsbProductName {
    /// Use custom provided string
    Custom(&'static str),
    /// Attempt to use string written to PFR location, using a default on failure.
    UsePfr,
}

pub struct UsbConfig {
    pub product_name: UsbProductName,
    pub manufacturer_name: &'static str,
    pub vid_pid: UsbVidPid,
}

pub struct Config {
    /// If provided, check secure and nonsecure versions in CFPA, and update if necessary.
    pub secure_firmware_version: Option<u32>,
    /// Enable NFC operation.
    pub nfc_enabled: bool,
    /// Panic if prince has not been provisioned in CFPA.
    pub require_prince: bool,
    /// If buttons are all activated for 5s, boot rom will boot.  Otherwise ignore.
    pub boot_to_bootrom: bool,
    /// For Usb initialization
    pub usb_config: Option<UsbConfig>,
}

/// For initializing the LPC55 runner safely.
pub struct Initializer {
    is_nfc_passive: bool,
    // hal: hal::Peripherals,
    syscon: hal::Syscon,
    pmc: hal::Pmc,
    anactrl: hal::Anactrl,
    config: Config,
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
    // NB: If this were to be re-used as card issuer's data in CCID ATR,
    // it would need to be limited or truncated to 13 bytes.
    "Solo 2 (custom)"
}

#[cfg(feature = "write-undefined-flash")]
/// This is necessary if prince encryption is enabled for the first time
/// after it was first provisioned.  In this case, there can be an exception
/// reading from undefined flash.  To fix, we run a pass over all filesystem
/// flash and set it to a defined value.
fn initialize_fs_flash(flash_gordon: &mut FlashGordon, prince: &mut hal::Prince<hal::typestates::init_state::Enabled>) {
    let page_count = ((631 * 1024 + 512) - build_constants::CONFIG_FILESYSTEM_BOUNDARY) / 512;

    let mut page_data = [0u8; 512];
    for page in 0 .. page_count {

        // With prince turned off, this should read as encrypted bytes.
        flash_gordon.read(build_constants::CONFIG_FILESYSTEM_BOUNDARY + page * 512, &mut page_data);

        // But if it's zero, then that means the data is undefined and it doesn't bother.
        if page_data == [0u8; 512] {
            info_now!("resetting page {}", page);
            // So we should write nonzero data to initialize flash.
            // We write it as encrypted, so it is in a known state when decrypted by the filesystem layer.
            page_data[0] = 1;
            flash_gordon.erase_page(build_constants::CONFIG_FILESYSTEM_BOUNDARY / 512 + page).ok();
            prince.write_encrypted(|prince| {
                prince.enable_region_2_for(||{
                    flash_gordon.write(build_constants::CONFIG_FILESYSTEM_BOUNDARY + page * 512, &page_data).unwrap();
                })
            });
        }
    }
}

impl Initializer {
    pub fn new(
        config: Config,
        syscon: hal::Syscon,
        pmc: hal::Pmc,
        anactrl: hal::Anactrl,
    ) -> Self {
        let is_nfc_passive = false;
        info_now!("making initializer");
        Self {
            is_nfc_passive,

            syscon,
            pmc,
            anactrl,

            config,
        }
    }

    fn enable_low_speed_for_passive_nfc(&mut self, mut iocon: hal::Iocon<hal::Enabled>, gpio: &mut hal::Gpio<hal::Enabled>)
        -> (hal::Iocon<hal::Enabled>, hal::Pin<board::nfc::NfcIrqPin, Gpio<direction::Input>>)
    {
        let nfc_irq = board::nfc::NfcIrqPin::take().unwrap().into_gpio_pin(&mut iocon, gpio).into_input();
        // Need to enable pullup for NFC IRQ input.
        let iocon = iocon.release();
        iocon.pio0_19.modify(|_,w| { w.mode().pull_up() } );
        let iocon = hal::Iocon::from(iocon).enabled(&mut self.syscon);
        let is_passive_mode = nfc_irq.is_low().ok().unwrap();

        self.is_nfc_passive = is_passive_mode;

        (iocon, nfc_irq)
    }

    fn enable_clocks(&mut self) -> Clocks {
        let anactrl = &mut self.anactrl;
        let pmc = &mut self.pmc;
        let syscon = &mut self.syscon;

        // Start out with slow clock if in passive mode;
        if self.is_nfc_passive {
            hal::ClockRequirements::default()
                .system_frequency(4.MHz())
                .configure(anactrl, pmc, syscon)
                .expect("Clock configuration failed")
        } else {
            hal::ClockRequirements::default()
                .system_frequency(96.MHz())
                .configure(anactrl, pmc, syscon)
                .expect("Clock configuration failed")
        }
    }

    fn is_bootrom_requested<T: Ctimer<hal::Enabled>>(&mut self, three_buttons: &board::ThreeButtons, timer: &mut Timer<T>) -> bool {
        // Boot to bootrom if buttons are all held for 5s
        timer.start(5_000_000.microseconds());
        while three_buttons.is_pressed(buttons::Button::A) &&
              three_buttons.is_pressed(buttons::Button::B) &&
              three_buttons.is_pressed(buttons::Button::Middle) {
            // info!("3 buttons pressed..");
            if timer.wait().is_ok() {
                return true;
            }
        }
        timer.cancel().ok();

        false
    }

    fn validate_cfpa(pfr: &mut Pfr<hal::Enabled>, current_version_maybe: Option<u32>, require_prince: bool) {
        let mut cfpa = pfr.read_latest_cfpa().unwrap();
        if let Some(current_version) = current_version_maybe {
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
        }

        if require_prince {
            #[cfg(not(feature = "no-encrypted-storage"))]
            assert!(
                cfpa.key_provisioned(hal::peripherals::pfr::KeyType::PrinceRegion2)
            );
        }
    }

    fn try_enable_fm11nc08 <T: Ctimer<hal::Enabled>>(
        &mut self,
        clocks: &Clocks,
        iocon: &mut hal::Iocon<hal::Enabled>,
        gpio: &mut hal::Gpio<hal::Enabled>,
        nfc_irq: hal::Pin<board::nfc::NfcIrqPin, Gpio<direction::Input>>,
        delay_timer: &mut Timer<T>,

        flexcomm0: hal::peripherals::flexcomm::Flexcomm0<Unknown>,
        inputmux: hal::peripherals::inputmux::InputMux<Unknown>,
        pint: hal::peripherals::pint::Pint<Unknown>,
    ) -> Option<board::nfc::NfcChip> {
        let token = clocks.support_flexcomm_token().unwrap();
        let syscon = &mut self.syscon;
        let spi = flexcomm0.enabled_as_spi(syscon, &token);

        // TODO save these so they can be released later
        let mut mux = inputmux.enabled(syscon);
        let mut pint = pint.enabled(syscon);
        pint.enable_interrupt(&mut mux, &nfc_irq, hal::peripherals::pint::Slot::Slot0, hal::peripherals::pint::Mode::ActiveLow);
        mux.disabled(syscon);

        let force_nfc_reconfig = cfg!(feature = "reconfigure-nfc");

        board::nfc::try_setup(
            spi,
            gpio,
            iocon,
            nfc_irq,
            delay_timer,
            force_nfc_reconfig,
        )

    }

    pub fn initialize_clocks(&mut self,
        iocon: hal::Iocon<Unknown>,
        gpio: hal::Gpio<Unknown>,
    ) -> stages::Clock {

        let syscon = &mut self.syscon;

        let mut iocon = iocon.enabled(syscon);
        let mut gpio = gpio.enabled(syscon);

        let nfc_irq = if self.config.nfc_enabled {
            let (new_iocon, nfc_irq) = self.enable_low_speed_for_passive_nfc(iocon, &mut gpio);
            iocon = new_iocon;
            Some(nfc_irq)
        } else {
            None
        };

        let clocks = self.enable_clocks();

        stages::Clock {
            nfc_irq,
            clocks,
            iocon,
            gpio,
            _clock: (),
        }

    }

    pub fn initialize_basic(&mut self,
        clock_stage: &mut stages::Clock,
        adc: hal::Adc<Unknown>,
        _dma: hal::Dma<Unknown>,
        delay_timer: ctimer::Ctimer0,
        ctimer1: ctimer::Ctimer1,
        ctimer2: ctimer::Ctimer2,
        _ctimer3: ctimer::Ctimer3,
        perf_timer: ctimer::Ctimer4,
        pfr: Pfr<Unknown>,
    ) -> stages::Basic {

        let clocks = clock_stage.clocks;

        let pmc = &mut self.pmc;
        let syscon = &mut self.syscon;

        // Start out with slow clock if in passive mode;
        #[allow(unused_mut)]
        let mut adc = Some(if self.is_nfc_passive {
            // important to start Adc early in passive mode
            hal::Adc::from(adc)
                .configure(board::clock_controller::DynamicClockController::adc_configuration())
                .enabled(pmc, syscon)
        } else {
            hal::Adc::from(adc)
                .enabled(pmc, syscon)
        });

        let mut delay_timer = Timer::new(delay_timer.enabled(syscon, clocks.support_1mhz_fro_token().unwrap()));
        let mut perf_timer = Timer::new(perf_timer.enabled(syscon, clocks.support_1mhz_fro_token().unwrap()));
        perf_timer.start(60_000_000.microseconds());

        let iocon = &mut clock_stage.iocon;
        let gpio = &mut clock_stage.gpio;

        let rgb = if !self.is_nfc_passive {
            #[cfg(feature = "board-lpcxpresso55")]
            let rgb = board::RgbLed::new(
                Pwm::new(ctimer2.enabled(syscon, clocks.support_1mhz_fro_token().unwrap())),
                iocon,
            );

            #[cfg(feature = "board-solo2")]
            let rgb = board::RgbLed::new(
                Pwm::new(_ctimer3.enabled(syscon, clocks.support_1mhz_fro_token().unwrap())),
                iocon,
            );

            Some(rgb)
        } else {
            None
        };

        let mut three_buttons = if !self.is_nfc_passive {
            #[cfg(feature = "board-lpcxpresso55")]
            let three_buttons = board::ThreeButtons::new(
                Timer::new(ctimer1.enabled(syscon, clocks.support_1mhz_fro_token().unwrap())),
                gpio,
                iocon,
            );

            #[cfg(feature = "board-solo2")]
            let three_buttons = {
                // TODO this should get saved somewhere to be released later.
                let mut dma = hal::Dma::from(_dma).enabled(syscon);

                board::ThreeButtons::new (
                    adc.take().unwrap(),
                    ctimer1.enabled(syscon, clocks.support_1mhz_fro_token().unwrap()),
                    ctimer2.enabled(syscon, clocks.support_1mhz_fro_token().unwrap()),
                    &mut dma,
                    clocks.support_touch_token().unwrap(),
                    gpio,
                    iocon,
                )
            };

            Some(three_buttons)
        } else {
            None
        };

        let mut pfr = pfr.enabled(&clocks).unwrap();
        Self::validate_cfpa(&mut pfr, self.config.secure_firmware_version, self.config.require_prince);

        if self.config.boot_to_bootrom {
            if let Some(three_buttons) = three_buttons.as_mut() {
                info!("bootrom request start {}", perf_timer.elapsed().0/1000);
                if self.is_bootrom_requested(three_buttons, &mut delay_timer) {
                    if let Some(mut rgb) = rgb {
                        // Give a small red blink show success
                        rgb.red(200); rgb.green(200); rgb.blue(0);
                    }
                    delay_timer.start(100_000.microseconds()); nb::block!(delay_timer.wait()).ok();

                    hal::boot_to_bootrom()
                }
            }
        }

        stages::Basic {
            delay_timer,
            perf_timer,
            pfr,

            adc,
            three_buttons,
            rgb,
        }
    }

    pub fn initialize_nfc(&mut self,
        clock_stage: &mut stages::Clock,
        basic_stage: &mut stages::Basic,
        flexcomm0: hal::peripherals::flexcomm::Flexcomm0<Unknown>,
        mux: hal::peripherals::inputmux::InputMux<Unknown>,
        pint: hal::peripherals::pint::Pint<Unknown>,
    ) -> stages::Nfc {

        let nfc_chip = if self.config.nfc_enabled {
            self.try_enable_fm11nc08(
                &clock_stage.clocks,
                &mut clock_stage.iocon,
                &mut clock_stage.gpio,
                clock_stage.nfc_irq.take().unwrap(),
                &mut basic_stage.delay_timer,
                flexcomm0,
                mux,
                pint,
            )
        } else {
            None
        };

        let mut iso14443: Option<nfc_device::Iso14443<board::nfc::NfcChip>> = None;

        let (contactless_requester, contactless_responder) = apdu_dispatch::interchanges::Contactless::claim()
            .expect("could not setup iso14443 ApduInterchange");

        if nfc_chip.is_some() {
            iso14443 = Some(nfc_device::Iso14443::new(
                nfc_chip.unwrap(), contactless_requester)
            )
        } else if self.is_nfc_passive {
            info!("Shouldn't get passive signal when there's no chip!");
        }

        if let Some(iso14443) = &mut iso14443 { iso14443.poll(); }
        if self.is_nfc_passive {
            // Give a small delay to charge up capacitors
            basic_stage.delay_timer.start(5_000.microseconds()); nb::block!(basic_stage.delay_timer.wait()).ok();
        }
        if let Some(iso14443) = &mut iso14443 { iso14443.poll(); }

        stages::Nfc {
            iso14443,
            contactless_responder: Some(contactless_responder),
        }

    }

    pub fn initialize_usb(
        &mut self,
        clock_stage: &mut stages::Clock,
        basic_stage: &mut stages::Basic,
        _usbhs: hal::peripherals::usbhs::Usbhs<Unknown>,
        _usbfs: hal::peripherals::usbfs::Usbfs<Unknown>,
    ) -> stages::Usb {
        let syscon = &mut self.syscon;
        let pmc = &mut self.pmc;
        let anactrl = &mut self.anactrl;

        let (contact_requester, contact_responder) = apdu_dispatch::interchanges::Contact::claim()
            .expect("could not setup ccid ApduInterchange");

        let (ctaphid_requester, ctaphid_responder) = ctaphid_dispatch::types::HidInterchange::claim()
            .expect("could not setup HidInterchange");

        info!("usb class start {} ms", basic_stage.perf_timer.elapsed().0/1000);

        let mut usb_classes: Option<types::UsbClasses> = None;

        if !self.is_nfc_passive {
            let iocon = &mut clock_stage.iocon;

            let usb_config = self.config.usb_config.take().unwrap();

            let usb0_vbus_pin = pins::Pio0_22::take().unwrap()
                .into_usb0_vbus_pin(iocon);

            #[cfg(not(feature = "usbfs-peripheral"))]
            let mut usbd = _usbhs.enabled_as_device(
                anactrl,
                pmc,
                syscon,
                &mut basic_stage.delay_timer,
                clock_stage.clocks.support_usbhs_token().unwrap(),
            );
            #[cfg(feature = "usbfs-peripheral")]
            let usbd = _usbfs.enabled_as_device(
                anactrl,
                pmc,
                syscon,
                clocks.support_usbfs_token().unwrap(),
            );
            #[cfg(not(any(feature = "highspeed", feature = "usbfs-peripheral")))]
            usbd.disable_high_speed();
            let _: types::EnabledUsbPeripheral = usbd;

            // ugh, what's the nice way?
            static mut USB_BUS: Option<usb_device::bus::UsbBusAllocator<UsbBus<types::EnabledUsbPeripheral>>> = None;
            unsafe { USB_BUS.replace(hal::drivers::UsbBus::new(usbd, usb0_vbus_pin)); }
            let usb_bus = unsafe { USB_BUS.as_ref().unwrap() };

            // our USB classes (must be allocated in order that they're passed in `.poll(...)` later!)
            //
            // NB: Card issuer's data can be at most 13 bytes (otherwise the constructor panics).
            // So for instance "Hacker Solo 2" would work, but "Solo 2 (custom)" would not.
            let ccid = usbd_ccid::Ccid::new(usb_bus, contact_requester, Some(b"Solo 2"));
            let current_time = basic_stage.perf_timer.elapsed().0/1000;
            let mut ctaphid = usbd_ctaphid::CtapHid::new(usb_bus, ctaphid_requester, current_time)
                .implements_ctap1()
                .implements_ctap2()
                .implements_wink();

            ctaphid.set_version(usbd_ctaphid::Version {
                major: crate::build_constants::CARGO_PKG_VERSION_MAJOR,
                minor: crate::build_constants::CARGO_PKG_VERSION_MINOR.to_be_bytes()[0],
                build: crate::build_constants::CARGO_PKG_VERSION_MINOR.to_be_bytes()[1],
            });

            // let serial = usbd_serial::SerialPort::new(usb_bus);

            // Only 16 bits, so take the upper bits of our semver
            let device_release =
                ((build_constants::CARGO_PKG_VERSION_MAJOR as u16) << 8) |
                (build_constants::CARGO_PKG_VERSION_MINOR as u16);

            // our composite USB device
            let product_string = match usb_config.product_name {
                UsbProductName::Custom(name) => name,
                UsbProductName::UsePfr => get_product_string(&mut basic_stage.pfr),
            };
            let serial_number = get_serial_number();

            let usbd = UsbDeviceBuilder::new(usb_bus, usb_config.vid_pid)
                .manufacturer(usb_config.manufacturer_name)
                .product(product_string)
                .serial_number(serial_number)
                .device_release(device_release)
                .max_packet_size_0(64)
                .composite_with_iads()
                .build();

            usb_classes = Some(types::UsbClasses::new(usbd, ccid, ctaphid));//, /*keyboard,*/ serial));

        }

        // Cancel any possible outstanding use in delay timing
        basic_stage.delay_timer.cancel().ok();

        stages::Usb {
            usb_classes,
            contact_responder: Some(contact_responder),
            ctaphid_responder: Some(ctaphid_responder),
        }
    }

    pub fn initialize_interfaces(&mut self, nfc_stage: &mut stages::Nfc, usb_stage: &mut stages::Usb) -> stages::Interfaces {

        info_now!("making interfaces");
        let apdu_dispatch = types::ApduDispatch::new(
            usb_stage.contact_responder.take().unwrap(),
            nfc_stage.contactless_responder.take().unwrap(),
        );
        let ctaphid_dispatch = types::CtaphidDispatch::new(
            usb_stage.ctaphid_responder.take().unwrap()
        );

        stages::Interfaces {
            apdu_dispatch,
            ctaphid_dispatch,
        }
    }

    pub fn initialize_flash(
        &mut self,
        rng: hal::peripherals::rng::Rng<Unknown>,
        prince: hal::peripherals::prince::Prince<Unknown>,
        flash: hal::peripherals::flash::Flash<Unknown>,
    ) -> stages::Flash {
        info_now!("making flash");
        let syscon = &mut self.syscon;

        #[allow(unused_mut)]
        let mut rng = rng.enabled(syscon);

        let prince = prince.enabled(&mut rng);
        prince.disable_all_region_2();

        let flash_gordon = Some(FlashGordon::new(flash.enabled(syscon)));

        stages::Flash {
            flash_gordon,
            prince: Some(prince),
            rng: Some(rng),
        }
    }

    pub fn initialize_filesystem(&mut self,
        clock_stage: &mut stages::Clock,
        basic_stage: &mut stages::Basic,
        nfc_stage: &mut stages::Nfc,
        flash_stage: &mut stages::Flash,
    ) -> stages::Filesystem {
        use littlefs2::fs::{Allocation, Filesystem};
        use types::{ExternalStorage, VolatileStorage};

        let syscon = &mut self.syscon;
        let pmc = &mut self.pmc;
        info_now!("making fs");

        #[allow(unused_mut)]
        let mut flash_gordon = flash_stage.flash_gordon.take().unwrap();

        #[cfg(not(feature = "no-encrypted-storage"))]
        let filesystem = {
            #[allow(unused_mut)]
            let mut prince = flash_stage.prince.take().unwrap();

            #[cfg(feature = "write-undefined-flash")]
            initialize_fs_flash(&mut flash_gordon, &mut prince);

            types::PrinceFilesystem::new(
                flash_gordon,
                prince,
            )
        };

        #[cfg(feature = "no-encrypted-storage")]
        let filesystem = types::PlainFilesystem::new(flash_gordon);

        // temporarily increase clock for the storage mounting or else it takes a long time.
        if self.is_nfc_passive {
            clock_stage.clocks = unsafe { hal::ClockRequirements::default()
                .system_frequency(48.MHz())
                .reconfigure(clock_stage.clocks, pmc, syscon) };
        }
        info_now!("mount start {} ms", basic_stage.perf_timer.elapsed().0/1000);
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

        let store = types::Store::claim().unwrap();

        if let Some(iso14443) = &mut nfc_stage.iso14443 { iso14443.poll(); }

        unsafe {

            INTERNAL_FS_ALLOC.as_mut().unwrap();
            INTERNAL_STORAGE.as_mut().unwrap();
            EXTERNAL_FS_ALLOC.as_mut().unwrap();
            VOLATILE_FS_ALLOC.as_mut().unwrap();
        }

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
            let rgb = basic_stage.rgb.as_mut().unwrap();
            rgb.blue(200);
            rgb.red(200);

            basic_stage.delay_timer.start(300_000.microseconds());
            nb::block!(basic_stage.delay_timer.wait()).ok();

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
            rgb.turn_off();
        }
        info!("mount end {} ms",basic_stage.perf_timer.elapsed().0/1000);

        // return to slow freq
        if self.is_nfc_passive {
            clock_stage.clocks = unsafe { hal::ClockRequirements::default()
                .system_frequency(12.MHz())
                .reconfigure(clock_stage.clocks, pmc, syscon) };
        }

        if let Some(iso14443) = &mut nfc_stage.iso14443 { iso14443.poll(); }

        // Cancel any possible outstanding use in delay timer
        basic_stage.delay_timer.cancel().ok();

        stages::Filesystem {
            store,
            internal_storage_fs: unsafe { &mut INTERNAL_STORAGE },
        }
    }

    pub fn initialize_trussed(
        &mut self,
        clock_stage: &mut stages::Clock,
        basic_stage: &mut stages::Basic,
        flash_stage: &mut stages::Flash,
        filesystem_stage: &mut stages::Filesystem,
        rtc: hal::peripherals::rtc::Rtc<Unknown>,
    ) -> types::Trussed {
        let syscon = &mut self.syscon;
        let pmc = &mut self.pmc;
        let clocks = clock_stage.clocks;

        let mut rtc = rtc.enabled(syscon, clocks.enable_32k_fro(pmc));
        rtc.reset();

        let rgb = if self.is_nfc_passive {
            None
        } else {
            basic_stage.rgb.take()
        };

        let three_buttons = basic_stage.three_buttons.take();

        let mut solobee_interface = board::trussed::UserInterface::new(rtc, three_buttons, rgb);
        solobee_interface.set_status(trussed::platform::ui::Status::Idle);

        let rng = flash_stage.rng.take().unwrap();
        let store = filesystem_stage.store;
        let board = types::Board::new(rng, store, solobee_interface);
        let trussed = trussed::service::Service::new(board);

        trussed
    }

    #[inline(never)]
    pub fn initialize_all(&mut self,
        iocon: hal::Iocon<Unknown>,
        gpio: hal::Gpio<Unknown>,

        adc: hal::Adc<Unknown>,
        dma: hal::peripherals::dma::Dma<Unknown>,
        delay_timer: ctimer::Ctimer0,
        ctimer1: ctimer::Ctimer1,
        ctimer2: ctimer::Ctimer2,
        ctimer3: ctimer::Ctimer3,
        perf_timer: ctimer::Ctimer4,
        pfr: Pfr<Unknown>,

        flexcomm0: hal::peripherals::flexcomm::Flexcomm0<Unknown>,
        mux: hal::peripherals::inputmux::InputMux<Unknown>,
        pint: hal::peripherals::pint::Pint<Unknown>,

        usbhs: hal::peripherals::usbhs::Usbhs<Unknown>,
        usbfs: hal::peripherals::usbfs::Usbfs<Unknown>,

        rng: hal::peripherals::rng::Rng<Unknown>,
        prince: hal::peripherals::prince::Prince<Unknown>,
        flash: hal::peripherals::flash::Flash<Unknown>,

        rtc: hal::peripherals::rtc::Rtc<Unknown>,
    ) -> stages::All {

        let mut clock_stage = self.initialize_clocks(iocon, gpio,);
        let mut basic_stage = self.initialize_basic(
            &mut clock_stage,
            adc,
            dma,
            delay_timer,
            ctimer1,
            ctimer2,
            ctimer3,
            perf_timer,
            pfr,
        );
        let mut nfc_stage = self.initialize_nfc(
            &mut clock_stage,
            &mut basic_stage,
            flexcomm0,
            mux,
            pint
        );

        let mut usb_stage = self.initialize_usb(
            &mut clock_stage,
            &mut basic_stage,
            usbhs,
            usbfs
        );
        let interfaces_stage = self.initialize_interfaces(&mut nfc_stage, &mut usb_stage);
        let mut flash_stage = self.initialize_flash(
            rng,
            prince,
            flash,
        );
        let mut filesystem_stage = self.initialize_filesystem(
            &mut clock_stage,
            &mut basic_stage,
            &mut nfc_stage,
            &mut flash_stage,
        );

        let trussed = self.initialize_trussed(
            &mut clock_stage,
            &mut basic_stage,
            &mut flash_stage,
            &mut filesystem_stage,
            rtc,
        );

        stages::All {
            trussed: trussed,
            filesystem: filesystem_stage,
            usb: usb_stage,
            interfaces: interfaces_stage,
            nfc: nfc_stage,
            basic: basic_stage,
            clock: clock_stage,
        }

    }

    /// Consumes the initializer -- must be done last.
    pub fn get_dynamic_clock_control(self, clock_stage: &mut stages::Clock, basic_stage: &mut stages::Basic)
    -> Option<clock_controller::DynamicClockController> {
        if self.is_nfc_passive {

            let adc = basic_stage.adc.take();
            let clocks = clock_stage.clocks;

            let pmc = self.pmc;
            let syscon = self.syscon;

            let gpio = &mut clock_stage.gpio;
            let iocon = &mut clock_stage.iocon;

            let mut new_clock_controller = clock_controller::DynamicClockController::new(adc.unwrap(),
                clocks, pmc, syscon, gpio, iocon);
            new_clock_controller.start_high_voltage_compare();

            Some(new_clock_controller)
        } else {
            None
        }
    }

    /// See if LPC55 will be in NFC passive operation.  Requires first initialization stage have been done.
    pub fn is_in_passive_operation(&self, _clock_stage: &stages::Clock)
    -> bool {
        return self.is_nfc_passive;
    }

}



