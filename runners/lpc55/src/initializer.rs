use defmt::info;

use crate::hal;
use hal::drivers::timer::Elapsed;
use hal::drivers::{clocks::Clocks, flash::FlashGordon, pins, pins::direction, Pwm, Timer, UsbBus};
use hal::peripherals::pfr::Pfr;
use hal::peripherals::{ctimer, ctimer::Ctimer};
use hal::prelude::*;
use hal::traits::wg::digital::v2::InputPin;
use hal::traits::wg::timer::Cancel;
use hal::typestates::init_state::Unknown;
use hal::typestates::pin::state::Gpio;
use static_cell::StaticCell;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};

use trussed::platform::UserInterface;

use board::traits::buttons;
use board::traits::buttons::Press;
use board::traits::rgb_led::RgbLed;

use crate::{build_constants, clock_controller, types};
use ref_swap::OptionRefSwap;
use trussed::interrupt::InterruptFlag;

pub mod stages;

/// Static APDU interchange channels (contactless = NFC, contact = USB/CCID).
static NFC_APDU_CHANNEL: apdu_dispatch::interchanges::Channel =
    apdu_dispatch::interchanges::Channel::new();
static USB_APDU_CHANNEL: apdu_dispatch::interchanges::Channel =
    apdu_dispatch::interchanges::Channel::new();
/// Static CTAPHID channel.
static CTAPHID_CHANNEL: ctaphid_dispatch::Channel<{ ctaphid_dispatch::DEFAULT_MESSAGE_SIZE }> =
    ctaphid_dispatch::Channel::new();
/// Static CTAPHID interrupt flag for coordinating between CTAPHID layer and apps.
static CTAPHID_INTERRUPT: OptionRefSwap<'static, InterruptFlag> = OptionRefSwap::new(None);

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
    static SERIAL_NUMBER: StaticCell<heapless::String<36>> = StaticCell::new();
    let serial_number = SERIAL_NUMBER.init(heapless::String::new());
    let uuid = crate::hal::uuid();
    use core::fmt::Write;
    // FIXME: figure out a different way to get a hex string here.
    serial_number
        .write_fmt(format_args!("{}", delog::hexstr!(&uuid)))
        .unwrap();
    serial_number
}

// SoloKeys stores a product string in the first 64 bytes of CMPA.
fn get_product_string(pfr: &mut Pfr<hal::typestates::init_state::Enabled>) -> &'static str {
    let data = pfr.cmpa_customer_data();

    // check the first 64 bytes of customer data for a string
    if data[0] != 0 {
        for i in 1..64 {
            if data[i] == 0 {
                let str_maybe = core::str::from_utf8(&data[0..i]);
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
fn initialize_fs_flash(
    flash_gordon: &mut FlashGordon,
    prince: &mut hal::Prince<hal::typestates::init_state::Enabled>,
) {
    let page_count = ((631 * 1024 + 512) - build_constants::CONFIG_FILESYSTEM_BOUNDARY) / 512;

    let mut page_data = [0u8; 512];
    for page in 0..page_count {
        // With prince turned off, this should read as encrypted bytes.
        flash_gordon.read(
            build_constants::CONFIG_FILESYSTEM_BOUNDARY + page * 512,
            &mut page_data,
        );

        // But if it's zero, then that means the data is undefined and it doesn't bother.
        if page_data == [0u8; 512] {
            info!("resetting page {}", page);
            // So we should write nonzero data to initialize flash.
            // We write it as encrypted, so it is in a known state when decrypted by the filesystem layer.
            page_data[0] = 1;
            flash_gordon
                .erase_page(build_constants::CONFIG_FILESYSTEM_BOUNDARY / 512 + page)
                .ok();
            prince.write_encrypted(|prince| {
                prince.enable_region_2_for(|| {
                    flash_gordon
                        .write(
                            build_constants::CONFIG_FILESYSTEM_BOUNDARY + page * 512,
                            &page_data,
                        )
                        .unwrap();
                })
            });
        }
    }
}

impl Initializer {
    pub fn new(config: Config, syscon: hal::Syscon, pmc: hal::Pmc, anactrl: hal::Anactrl) -> Self {
        let is_nfc_passive = false;
        info!("making initializer");
        Self {
            is_nfc_passive,

            syscon,
            pmc,
            anactrl,

            config,
        }
    }

    fn enable_low_speed_for_passive_nfc(
        &mut self,
        mut iocon: hal::Iocon<hal::Enabled>,
        gpio: &mut hal::Gpio<hal::Enabled>,
    ) -> (
        hal::Iocon<hal::Enabled>,
        hal::Pin<board::nfc::NfcIrqPin, Gpio<direction::Input>>,
    ) {
        let nfc_irq = board::nfc::NfcIrqPin::take()
            .unwrap()
            .into_gpio_pin(&mut iocon, gpio)
            .into_input();
        // Need to enable pullup for NFC IRQ input.
        let iocon = iocon.release();
        iocon.pio0_19.modify(|_, w| w.mode().pull_up());
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

    fn is_bootrom_requested<T: Ctimer<hal::Enabled>>(
        &mut self,
        three_buttons: &board::ThreeButtons,
        timer: &mut Timer<T>,
    ) -> bool {
        // Boot to bootrom if buttons are all held for 5s
        timer.start(5_000_000.microseconds());
        while three_buttons.is_pressed(buttons::Button::A)
            && three_buttons.is_pressed(buttons::Button::B)
            && three_buttons.is_pressed(buttons::Button::Middle)
        {
            // info!("3 buttons pressed..");
            if timer.wait().is_ok() {
                return true;
            }
        }
        timer.cancel().ok();

        false
    }

    fn validate_cfpa(
        pfr: &mut Pfr<hal::Enabled>,
        current_version_maybe: Option<u32>,
        require_prince: bool,
    ) {
        let mut cfpa = pfr.read_latest_cfpa().unwrap();
        if let Some(current_version) = current_version_maybe {
            if cfpa.secure_fw_version < current_version || cfpa.ns_fw_version < current_version {
                info!(
                    "updating cfpa from {} to {}",
                    cfpa.secure_fw_version, current_version
                );

                // All of these are monotonic counters.
                cfpa.version += 1;
                cfpa.secure_fw_version = current_version;
                cfpa.ns_fw_version = current_version;
                pfr.write_cfpa(&cfpa).unwrap();
            } else {
                info!(
                    "do not need to update cfpa version {}",
                    cfpa.secure_fw_version
                );
            }
        }

        if require_prince {
            #[cfg(not(feature = "no-encrypted-storage"))]
            assert!(cfpa.key_provisioned(hal::peripherals::pfr::KeyType::PrinceRegion2));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn try_enable_fm11nc08<T: Ctimer<hal::Enabled>>(
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

        // Install the shared Spi0 bus before NFC or flash bring-up; both reach
        // it through `shared_spi::BusProxy`.
        board::shared_spi::setup(spi, iocon);

        // TODO save these so they can be released later
        let mut mux = inputmux.enabled(syscon);
        let mut pint = pint.enabled(syscon);
        pint.enable_interrupt(
            &mut mux,
            &nfc_irq,
            hal::peripherals::pint::Slot::Slot0,
            hal::peripherals::pint::Mode::ActiveLow,
        );
        mux.disabled(syscon);

        let force_nfc_reconfig = cfg!(feature = "reconfigure-nfc");

        board::nfc::try_setup(gpio, iocon, nfc_irq, delay_timer, force_nfc_reconfig)
    }

    pub fn initialize_clocks(
        &mut self,
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
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn initialize_basic(
        &mut self,
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
            adc.configure(board::clock_controller::DynamicClockController::adc_configuration())
                .enabled(pmc, syscon)
        } else {
            adc.enabled(pmc, syscon)
        });

        let mut delay_timer =
            Timer::new(delay_timer.enabled(syscon, clocks.support_1mhz_fro_token().unwrap()));
        let mut perf_timer =
            Timer::new(perf_timer.enabled(syscon, clocks.support_1mhz_fro_token().unwrap()));
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
                let mut dma = _dma.enabled(syscon);

                board::ThreeButtons::new(
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
        Self::validate_cfpa(
            &mut pfr,
            self.config.secure_firmware_version,
            self.config.require_prince,
        );

        if self.config.boot_to_bootrom {
            if let Some(three_buttons) = three_buttons.as_mut() {
                info!("bootrom request start {}", perf_timer.elapsed().0 / 1000);
                if self.is_bootrom_requested(three_buttons, &mut delay_timer) {
                    if let Some(mut rgb) = rgb {
                        // Give a small red blink show success
                        rgb.red(200);
                        rgb.green(200);
                        rgb.blue(0);
                    }
                    delay_timer.start(100_000.microseconds());
                    nb::block!(delay_timer.wait()).ok();

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

    pub fn initialize_nfc(
        &mut self,
        clock_stage: &mut stages::Clock,
        basic_stage: &mut stages::Basic,
        #[allow(unused_variables)] flexcomm0: hal::peripherals::flexcomm::Flexcomm0<Unknown>,
        flexcomm1: board::nfc_i2c::NfcFlexcomm,
        #[allow(unused_variables)] mux: hal::peripherals::inputmux::InputMux<Unknown>,
        #[allow(unused_variables)] pint: hal::peripherals::pint::Pint<Unknown>,
    ) -> stages::Nfc {
        let (contactless_requester, contactless_responder) = NFC_APDU_CHANNEL
            .split()
            .expect("could not setup iso14443 ApduInterchange");

        // NFC chip autodetect: the FM11NT082C ACKs address 0x57 on the board's I2C
        // bus; otherwise fall back to the FM11NC08 on Flexcomm0 SPI (which itself
        // returns None if its chip is absent, so there's no separate "no NFC" branch).
        let nfc_chip: Option<board::nfc::NfcFrontend> = if self.config.nfc_enabled {
            let mut i2c = board::nfc_i2c::BoundedI2c::setup(
                flexcomm1,
                &clock_stage.clocks,
                &mut self.syscon,
                &mut clock_stage.iocon,
            );
            if i2c.probe(0x57) {
                // Take the board's FM11 IRQ pin (EVK PIO1_22 = P20 pin 8; solo
                // PIO0_19) and set up a PINT (Slot0 -> the nfc_irq task, active-low)
                // so the chip's RxDone interrupt drives reads.
                #[cfg(feature = "board-lpcxpresso55")]
                let int = {
                    let int = pins::Pio1_22::take()
                        .unwrap()
                        .into_gpio_pin(&mut clock_stage.iocon, &mut clock_stage.gpio)
                        .into_input();
                    // The FM11 IRQ is open-drain active-low; `into_gpio_pin` sets
                    // no pull, so enable the internal pull-up.
                    unsafe { &*hal::raw::IOCON::ptr() }
                        .pio1_22
                        .modify(|_, w| w.mode().pull_up());
                    int
                };
                #[cfg(not(feature = "board-lpcxpresso55"))]
                let int = clock_stage.nfc_irq.take().unwrap();

                let mut mux = mux.enabled(&mut self.syscon);
                let mut pint = pint.enabled(&mut self.syscon);
                pint.enable_interrupt(
                    &mut mux,
                    &int,
                    hal::peripherals::pint::Slot::Slot0,
                    hal::peripherals::pint::Mode::ActiveLow,
                );
                mux.disabled(&mut self.syscon);

                board::nfc::try_setup_082c(i2c, int, &mut basic_stage.delay_timer).map(|chip| {
                    // Enable idle's slow NFC poll fallback only for the 082C (see
                    // `NFC_IDLE_POLL`); legacy NC08 boards stay IRQ-driven.
                    crate::NFC_IDLE_POLL.store(true, core::sync::atomic::Ordering::Relaxed);
                    board::nfc::NfcFrontend::Fm11nt082c(chip)
                })
            } else {
                // No 082C on I2C → FM11NC08 on Flexcomm0 SPI.
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
                .map(board::nfc::NfcFrontend::Fm11nc08)
            }
        } else {
            None
        };

        let mut iso14443: Option<types::Iso14443> = None;

        if let Some(chip) = nfc_chip {
            iso14443 = Some(nfc_device::Iso14443::new(chip, contactless_requester))
        } else if self.is_nfc_passive {
            info!("Shouldn't get passive signal when there's no chip!");
        }

        if let Some(iso14443) = &mut iso14443 {
            iso14443.poll();
        }
        if self.is_nfc_passive {
            // Give a small delay to charge up capacitors
            basic_stage.delay_timer.start(5_000.microseconds());
            nb::block!(basic_stage.delay_timer.wait()).ok();
        }
        if let Some(iso14443) = &mut iso14443 {
            iso14443.poll();
        }

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
        #[cfg(feature = "admin-app")] store: types::RunnerStore,
    ) -> stages::Usb {
        let syscon = &mut self.syscon;
        let pmc = &mut self.pmc;
        let anactrl = &mut self.anactrl;

        let (contact_requester, contact_responder) = USB_APDU_CHANNEL
            .split()
            .expect("could not setup ccid ApduInterchange");

        let (ctaphid_requester, ctaphid_responder) = CTAPHID_CHANNEL
            .split()
            .expect("could not setup HidInterchange");

        info!(
            "usb class start {} ms",
            basic_stage.perf_timer.elapsed().0 / 1000
        );

        let mut usb_classes: Option<types::UsbClasses> = None;
        #[cfg(feature = "wallet")]
        let mut wallet_responder: Option<types::WalletResponder> = None;

        if !self.is_nfc_passive {
            let iocon = &mut clock_stage.iocon;

            let usb_config = self.config.usb_config.take().unwrap();

            let usb0_vbus_pin = pins::Pio0_22::take().unwrap().into_usb0_vbus_pin(iocon);

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

            static USB_BUS: StaticCell<
                usb_device::bus::UsbBusAllocator<UsbBus<types::EnabledUsbPeripheral>>,
            > = StaticCell::new();
            let usb_bus = USB_BUS.init(hal::drivers::UsbBus::new(usbd, usb0_vbus_pin));

            // our USB classes (must be allocated in order that they're passed in `.poll(...)` later!)
            // Interface numbers are assigned in allocation order; the Wallet HID
            // class is allocated FIRST so it lands on interface 0, which the
            // agave `solana` CLI requires (it matches HID by
            // `usage_page == 0xFF00 || interface_number == 0`).
            //
            // Allocated before the `UsbDeviceBuilder::build()` below, since
            // usb-device requires every class to claim its endpoints before the
            // device is built.
            #[cfg(feature = "wallet")]
            let wallet_hid = {
                let (wallet_rq, wallet_rp) = types::WALLET_HID_CHANNEL
                    .split()
                    .expect("wallet hid channel already split");
                wallet_responder = Some(wallet_rp);
                wallet_app::usbd::WalletHid::new(usb_bus, wallet_rq)
            };

            // NB: Card issuer's data can be at most 13 bytes (otherwise the constructor panics).
            // So for instance "Hacker Solo 2" would work, but "Solo 2 (custom)" would not.
            let ccid = usbd_ccid::Ccid::new(usb_bus, contact_requester, Some(b"Solo 2"));

            let current_time = basic_stage.perf_timer.elapsed().0 / 1000;
            let mut ctaphid = usbd_ctaphid::CtapHid::with_interrupt(
                usb_bus,
                ctaphid_requester,
                Some(&CTAPHID_INTERRUPT),
                current_time,
            )
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
            let device_release = ((build_constants::CARGO_PKG_VERSION_MAJOR as u16) << 8)
                | build_constants::CARGO_PKG_VERSION_MINOR;

            // our composite USB device
            let default_product = match usb_config.product_name {
                UsbProductName::Custom(name) => name,
                UsbProductName::UsePfr => get_product_string(&mut basic_stage.pfr),
            };
            let serial_number = get_serial_number();

            // A persisted DeviceConfig (admin-app SET_CONFIG) drives the status
            // LED on every build; the USB vid/pid + descriptor strings are only
            // overridden on a `hacker` build (secure stays the firmware default).
            #[cfg(feature = "admin-app")]
            let (vid_pid, manufacturer_string, product_string) = {
                let mut fs = trussed::store::filestore::ClientFilestore::new(
                    littlefs2::path!("admin").into(),
                    store,
                );
                let dc = admin_app::config::load::<_, crate::device_config::DeviceConfig>(&mut fs)
                    .unwrap_or_default();
                // Push the configured status-LED colors to the board UI.
                use core::sync::atomic::Ordering;
                board::trussed::LED_IDLE_RGB.store(dc.led.idle, Ordering::Relaxed);
                board::trussed::LED_UP_RGB.store(dc.led.up, Ordering::Relaxed);

                #[cfg(feature = "hacker")]
                {
                    let manufacturer_string: &'static str = if dc.usb.manufacturer.is_empty() {
                        usb_config.manufacturer_name
                    } else {
                        static M: StaticCell<admin_app::ConfigString> = StaticCell::new();
                        M.init(dc.usb.manufacturer)
                    };
                    let product_string: &'static str = if dc.usb.product.is_empty() {
                        default_product
                    } else {
                        static P: StaticCell<admin_app::ConfigString> = StaticCell::new();
                        P.init(dc.usb.product)
                    };
                    (
                        UsbVidPid(dc.usb.vid, dc.usb.pid),
                        manufacturer_string,
                        product_string,
                    )
                }
                #[cfg(not(feature = "hacker"))]
                {
                    (
                        usb_config.vid_pid,
                        usb_config.manufacturer_name,
                        default_product,
                    )
                }
            };
            #[cfg(not(feature = "admin-app"))]
            let (vid_pid, manufacturer_string, product_string) = (
                usb_config.vid_pid,
                usb_config.manufacturer_name,
                default_product,
            );

            let usbd = UsbDeviceBuilder::new(usb_bus, vid_pid)
                .manufacturer(manufacturer_string)
                .product(product_string)
                .serial_number(serial_number)
                .device_release(device_release)
                .max_packet_size_0(64)
                .composite_with_iads()
                .build();

            usb_classes = Some(types::UsbClasses::new(
                usbd,
                ccid,
                ctaphid,
                #[cfg(feature = "wallet")]
                wallet_hid,
            ));
        }

        // Cancel any possible outstanding use in delay timing
        basic_stage.delay_timer.cancel().ok();

        stages::Usb {
            usb_classes,
            contact_responder: Some(contact_responder),
            ctaphid_responder: Some(ctaphid_responder),
            #[cfg(feature = "wallet")]
            wallet_responder,
        }
    }

    pub fn initialize_interfaces(
        &mut self,
        nfc_stage: &mut stages::Nfc,
        usb_stage: &mut stages::Usb,
    ) -> stages::Interfaces {
        info!("making interfaces");
        let apdu_dispatch = types::ApduDispatch::new(
            usb_stage.contact_responder.take().unwrap(),
            nfc_stage.contactless_responder.take().unwrap(),
        );
        let ctaphid_dispatch = types::CtaphidDispatch::with_interrupt(
            usb_stage.ctaphid_responder.take().unwrap(),
            Some(&CTAPHID_INTERRUPT),
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
        info!("making flash");
        let syscon = &mut self.syscon;

        #[allow(unused_mut)]
        let mut rng = rng.enabled(syscon);

        let prince = prince.enabled(&rng);
        prince.disable_all_region_2();

        let flash_gordon = Some(FlashGordon::new(flash.enabled(syscon)));

        stages::Flash {
            flash_gordon,
            prince: Some(prince),
            rng: Some(rng),
        }
    }

    pub fn initialize_filesystem(
        &mut self,
        clock_stage: &mut stages::Clock,
        basic_stage: &mut stages::Basic,
        nfc_stage: &mut stages::Nfc,
        flash_stage: &mut stages::Flash,
    ) -> stages::Filesystem {
        use littlefs2::fs::{Allocation, Filesystem};
        use types::{ExternalStorage, VolatileStorage};

        let syscon = &mut self.syscon;
        let pmc = &mut self.pmc;
        info!("making fs");

        #[allow(unused_mut)]
        let mut flash_gordon = flash_stage.flash_gordon.take().unwrap();

        #[cfg(not(feature = "no-encrypted-storage"))]
        let filesystem = {
            #[allow(unused_mut)]
            let mut prince = flash_stage.prince.take().unwrap();

            #[cfg(feature = "write-undefined-flash")]
            initialize_fs_flash(&mut flash_gordon, &mut prince);

            types::PrinceFilesystem::new(flash_gordon, prince)
        };

        #[cfg(feature = "no-encrypted-storage")]
        let filesystem = types::PlainFilesystem::new(flash_gordon);

        // temporarily increase clock for the storage mounting or else it takes a long time.
        if self.is_nfc_passive {
            clock_stage.clocks = unsafe {
                hal::ClockRequirements::default()
                    .system_frequency(48.MHz())
                    .reconfigure(clock_stage.clocks, pmc, syscon)
            };
        }
        info!(
            "mount start {} ms",
            basic_stage.perf_timer.elapsed().0 / 1000
        );

        // Convert all refs to raw pointers to allow retry after formatting.
        // Only one derived &mut ref is live at a time (safe by construction).
        static INTERNAL_STORAGE: StaticCell<types::FlashStorage> = StaticCell::new();
        let internal_storage = INTERNAL_STORAGE.init(filesystem) as *mut types::FlashStorage;
        static INTERNAL_FS_ALLOC: StaticCell<Allocation<types::FlashStorage>> = StaticCell::new();
        let internal_fs_alloc =
            INTERNAL_FS_ALLOC.init(Filesystem::allocate()) as *mut Allocation<types::FlashStorage>;

        static VOLATILE_STORAGE: StaticCell<VolatileStorage> = StaticCell::new();
        let volatile_storage =
            VOLATILE_STORAGE.init(VolatileStorage::new()) as *mut VolatileStorage;
        static VOLATILE_FS_ALLOC: StaticCell<Allocation<VolatileStorage>> = StaticCell::new();
        let volatile_fs_alloc =
            VOLATILE_FS_ALLOC.init(Filesystem::allocate()) as *mut Allocation<VolatileStorage>;

        if let Some(iso14443) = &mut nfc_stage.iso14443 {
            iso14443.poll();
        }

        // Try to mount internal FS without formatting.  If it fails (or format-filesystem
        // feature is set), format all three filesystems then re-mount.
        let needs_format = Filesystem::mount(unsafe { &mut *internal_fs_alloc }, unsafe {
            &mut *internal_storage
        })
        .is_err();
        let needs_format = needs_format || cfg!(feature = "format-filesystem");

        if needs_format {
            if let Some(rgb) = basic_stage.rgb.as_mut() {
                rgb.blue(200);
                rgb.red(200);
            }
            basic_stage.delay_timer.start(300_000.microseconds());
            nb::block!(basic_stage.delay_timer.wait()).ok();

            info!("Not yet formatted!  Formatting..");
            Filesystem::format(unsafe { &mut *internal_storage }).unwrap();
            Filesystem::format(unsafe { &mut *volatile_storage }).unwrap();

            if let Some(rgb) = basic_stage.rgb.as_mut() {
                rgb.turn_off();
            }
        }

        // Final mounts.  Internal was either already formatted or just formatted above.
        // External and volatile are RAM-based; format on first use if needed.
        static INTERNAL_FS: StaticCell<Filesystem<'static, types::FlashStorage>> =
            StaticCell::new();
        let internal_fs: &'static mut Filesystem<'static, types::FlashStorage> = INTERNAL_FS.init(
            Filesystem::mount(unsafe { &mut *internal_fs_alloc }, unsafe {
                &mut *internal_storage
            })
            .unwrap(),
        );

        // Active mode: probe the GD25Q16 and mount it as the external FS. If
        // absent (EVK without the chip, dead chip) or in passive mode (chip
        // left unpowered to save field energy), fall back to a RAM-backed
        // stand-in so the device still enumerates and is reachable — never
        // panics, never bricks the device.
        let external_fs: &'static dyn trussed::store::DynFilesystem = {
            use types::ExternalFallbackStorage;

            static EXT_CHIP_STORAGE: StaticCell<ExternalStorage> = StaticCell::new();
            static EXT_CHIP_ALLOC: StaticCell<Allocation<ExternalStorage>> = StaticCell::new();
            static EXT_CHIP_FS: StaticCell<Filesystem<'static, ExternalStorage>> =
                StaticCell::new();
            static EXT_RAM_STORAGE: StaticCell<ExternalFallbackStorage> = StaticCell::new();
            static EXT_RAM_ALLOC: StaticCell<Allocation<ExternalFallbackStorage>> =
                StaticCell::new();
            static EXT_RAM_FS: StaticCell<Filesystem<'static, ExternalFallbackStorage>> =
                StaticCell::new();

            // Passive (RF-powered) can't afford the flash chip's draw + boot-time
            // mount reads — it browns out the field and NFC stops. So passive runs
            // on the RAM fallback with the chip left unpowered; only active/USB
            // probes and uses the flash.
            let (chip, _selftest) = if self.is_nfc_passive {
                (None, board::flash::SelftestResult::ZERO)
            } else {
                board::flash::try_setup(
                    &mut clock_stage.gpio,
                    &mut clock_stage.iocon,
                    &mut basic_stage.delay_timer,
                )
            };

            match chip {
                Some(chip) => {
                    info!("external flash: GD25Q16 detected, using chip");
                    let storage = EXT_CHIP_STORAGE.init(chip) as *mut ExternalStorage;
                    let alloc = EXT_CHIP_ALLOC.init(Filesystem::allocate())
                        as *mut Allocation<ExternalStorage>;
                    if needs_format {
                        info!("wiping external FS (internal reformat or format-filesystem)");
                        Filesystem::format(unsafe { &mut *storage }).unwrap();
                    }
                    let f =
                        match Filesystem::mount(unsafe { &mut *alloc }, unsafe { &mut *storage }) {
                            Ok(fs) => fs,
                            Err(_) => {
                                Filesystem::format(unsafe { &mut *storage }).unwrap();
                                Filesystem::mount(unsafe { &mut *alloc }, unsafe { &mut *storage })
                                    .unwrap()
                            }
                        };
                    EXT_CHIP_FS.init(f) as &'static dyn trussed::store::DynFilesystem
                }
                None => {
                    defmt::warn!("external flash absent / JEDEC mismatch — RAM fallback");
                    // Brief red flash so a developer watching a sealed
                    // Solo 2 (no JTAG / no serial) can tell the chip was
                    // not detected. Trussed's UI loop overwrites this
                    // with breathing-green idle shortly after.
                    if let Some(rgb) = basic_stage.rgb.as_mut() {
                        rgb.red(200);
                        rgb.green(0);
                        rgb.blue(0);
                        basic_stage.delay_timer.start(250_000.microseconds());
                        let _ = nb::block!(basic_stage.delay_timer.wait());
                        rgb.turn_off();
                    }
                    let storage = EXT_RAM_STORAGE.init(ExternalFallbackStorage::new())
                        as *mut ExternalFallbackStorage;
                    let alloc = EXT_RAM_ALLOC.init(Filesystem::allocate())
                        as *mut Allocation<ExternalFallbackStorage>;
                    let f =
                        match Filesystem::mount(unsafe { &mut *alloc }, unsafe { &mut *storage }) {
                            Ok(fs) => fs,
                            Err(_) => {
                                Filesystem::format(unsafe { &mut *storage }).unwrap();
                                Filesystem::mount(unsafe { &mut *alloc }, unsafe { &mut *storage })
                                    .unwrap()
                            }
                        };
                    EXT_RAM_FS.init(f) as &'static dyn trussed::store::DynFilesystem
                }
            }
        };

        static VOLATILE_FS: StaticCell<Filesystem<'static, VolatileStorage>> = StaticCell::new();
        let volatile_fs: &'static mut Filesystem<'static, VolatileStorage> = VOLATILE_FS.init({
            match Filesystem::mount(unsafe { &mut *volatile_fs_alloc }, unsafe {
                &mut *volatile_storage
            }) {
                Ok(fs) => fs,
                Err(_) => {
                    Filesystem::format(unsafe { &mut *volatile_storage }).unwrap();
                    Filesystem::mount(unsafe { &mut *volatile_fs_alloc }, unsafe {
                        &mut *volatile_storage
                    })
                    .unwrap()
                }
            }
        });

        info!("mount end {} ms", basic_stage.perf_timer.elapsed().0 / 1000);

        // return to slow freq
        if self.is_nfc_passive {
            clock_stage.clocks = unsafe {
                hal::ClockRequirements::default()
                    .system_frequency(12.MHz())
                    .reconfigure(clock_stage.clocks, pmc, syscon)
            };
        }

        if let Some(iso14443) = &mut nfc_stage.iso14443 {
            iso14443.poll();
        }

        // Cancel any possible outstanding use in delay timer
        basic_stage.delay_timer.cancel().ok();

        let store = types::RunnerStore::new(internal_fs, external_fs, volatile_fs);

        // Test-only FIDO2 attestation provisioning. Mirrors the DK runner's
        // boot-time provisioning: bakes the public Nitrokey FIDO test PKI
        // into the binary and writes it to LFS on first boot. Without
        // this, CTAP1 `Register` and CTAP2 `MakeCredential` return
        // `KeyReferenceNotFound (0x6A88)` and the `tests/fido2::u2f::*`
        // suite fails. Gated by `test-up-control` so production builds
        // never include the test key.
        #[cfg(feature = "test-up-control")]
        {
            use trussed::store::Store as _;
            const ATTESTATION_CERT: &[u8] = include_bytes!("../../pc/data/fido-cert.der");
            const ATTESTATION_KEY: &[u8] = include_bytes!("../../pc/data/fido-key.trussed");
            let ifs = store.ifs();
            if !ifs.exists(littlefs2::path!("fido/x5c/00"))
                || !ifs.exists(littlefs2::path!("fido/sec/00"))
            {
                let _ = ifs.create_dir_all(littlefs2::path!("fido/x5c"));
                let _ = ifs.create_dir_all(littlefs2::path!("fido/sec"));
                let _ = ifs.write(littlefs2::path!("fido/x5c/00"), ATTESTATION_CERT);
                let _ = ifs.write(littlefs2::path!("fido/sec/00"), ATTESTATION_KEY);
            }
        }

        stages::Filesystem {
            store,
            internal_storage_fs: internal_storage,
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

        // Buttons stay in `basic_stage` (hoisted to the idle loop by the
        // runner); the UI only needs to know whether they're present.
        let has_buttons = basic_stage.three_buttons.is_some();

        let mut solobee_interface = board::trussed::UserInterface::new(rtc, has_buttons, rgb);
        solobee_interface.set_status(trussed::platform::ui::Status::Idle);

        let rng = flash_stage.rng.take().unwrap();
        let store = filesystem_stage.store;
        let board = types::Board::new(rng, store, solobee_interface);
        let service = trussed::service::Service::with_dispatch(board, types::Dispatch::default());

        types::Trussed::new(service)
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    pub fn initialize_all(
        &mut self,
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
        flexcomm1: board::nfc_i2c::NfcFlexcomm,
        mux: hal::peripherals::inputmux::InputMux<Unknown>,
        pint: hal::peripherals::pint::Pint<Unknown>,

        usbhs: hal::peripherals::usbhs::Usbhs<Unknown>,
        usbfs: hal::peripherals::usbfs::Usbfs<Unknown>,

        rng: hal::peripherals::rng::Rng<Unknown>,
        prince: hal::peripherals::prince::Prince<Unknown>,
        flash: hal::peripherals::flash::Flash<Unknown>,

        rtc: hal::peripherals::rtc::Rtc<Unknown>,
    ) -> stages::All {
        let mut clock_stage = self.initialize_clocks(iocon, gpio);
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
            flexcomm1,
            mux,
            pint,
        );

        // Flash + filesystem come up before USB so the USB descriptor (vid/pid)
        // can be chosen from persisted config on the mounted FS at enumeration
        // time. NB: this puts the (slow, occasionally-hang-prone) FS mount ahead
        // of USB — only safe where a corrupt-FS hang is recoverable (EVK / a key
        // with a PIO0_5 button).
        let mut flash_stage = self.initialize_flash(rng, prince, flash);
        let mut filesystem_stage = self.initialize_filesystem(
            &mut clock_stage,
            &mut basic_stage,
            &mut nfc_stage,
            &mut flash_stage,
        );
        let mut usb_stage = self.initialize_usb(
            &mut clock_stage,
            &mut basic_stage,
            usbhs,
            usbfs,
            #[cfg(feature = "admin-app")]
            filesystem_stage.store,
        );
        let interfaces_stage = self.initialize_interfaces(&mut nfc_stage, &mut usb_stage);

        let trussed = self.initialize_trussed(
            &mut clock_stage,
            &mut basic_stage,
            &mut flash_stage,
            &mut filesystem_stage,
            rtc,
        );

        stages::All {
            trussed,
            filesystem: filesystem_stage,
            usb: usb_stage,
            interfaces: interfaces_stage,
            nfc: nfc_stage,
            basic: basic_stage,
            clock: clock_stage,
        }
    }

    /// Consumes the initializer -- must be done last.
    pub fn get_dynamic_clock_control(
        self,
        clock_stage: &mut stages::Clock,
        basic_stage: &mut stages::Basic,
    ) -> Option<clock_controller::DynamicClockController> {
        if self.is_nfc_passive {
            let adc = basic_stage.adc.take();
            let clocks = clock_stage.clocks;

            let pmc = self.pmc;
            let syscon = self.syscon;

            let gpio = &mut clock_stage.gpio;
            let iocon = &mut clock_stage.iocon;

            let mut new_clock_controller = clock_controller::DynamicClockController::new(
                adc.unwrap(),
                clocks,
                pmc,
                syscon,
                gpio,
                iocon,
            );
            new_clock_controller.start_high_voltage_compare();

            Some(new_clock_controller)
        } else {
            None
        }
    }

    /// See if LPC55 will be in NFC passive operation.  Requires first initialization stage have been done.
    pub fn is_in_passive_operation(&self, _clock_stage: &stages::Clock) -> bool {
        self.is_nfc_passive
    }
}
