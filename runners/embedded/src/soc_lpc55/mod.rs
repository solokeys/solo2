pub mod clock_controller;
pub mod nfc;
pub mod traits;
pub mod trussed;
pub mod types;

use lpc55_hal::{Anactrl, Iocon, Pmc, Syscon, drivers::Timer};
use lpc55_hal::drivers::{clocks::Clocks, pins};
use lpc55_hal::typestates::init_state::{Enabled, Unknown};
use lpc55_hal::typestates::pin::gpio::direction;

/*
   Rust being ridiculous, episode #14728.

   For brevity and because it stacks nicely, we would like to write
   the following - it captures exactly what we want and it's elegant
   and perfectly readable:

#[cfg_attr(feature = "board-nk3am", path = "board_nk3am.rs")]
#[cfg_attr(feature = "board-solo2", path = "board_solo2.rs")]
#[cfg_attr(feature = "board-nk3xn", path = "board_nk3xn.rs")]
pub mod board;

   However, due to this PR[1], the presence of a path attribute changes
   the way that nested modules (i.e. those inside the module with that
   attribute) are looked up. With the attribute, rustc doesn't make
   "board-nk3xn" and friends "directory owners", so all nested modules
   are expected to be in the same directory.

   And even though there's ample documentation for the module subsystem
   and the effects of the path attribute[2], this aspect isn't mentioned.
   Even worse (and also quite customary for Rust), there's a bug report[3]
   open since 2019 for adding exactly that.

   There's even a simple fix! See below for the magic special case.

[1]: https://github.com/rust-lang/rust/pull/37602
[2]: https://doc.rust-lang.org/reference/items/modules.html
[3]: https://github.com/rust-lang/reference/issues/573
 */

// modules with path attribute *are* directory owners if their path
// refers to a 'mod.rs'
#[cfg_attr(feature = "board-nk3am", path = "board_nk3am/mod.rs")]
#[cfg_attr(feature = "board-solo2", path = "board_solo2/mod.rs")]
#[cfg_attr(feature = "board-nk3xn", path = "board_nk3xn/mod.rs")]
pub mod board;

pub fn init_bootup(iocon: &mut lpc55_pac::IOCON) {
	unsafe { types::DEVICE_UUID.copy_from_slice(&lpc55_hal::uuid()); };

	/* configure the NFC IRQ pullup pin now, before the HAL consumes the devices */
	// #[cfg(feature = "nfc")]
	iocon.pio0_19.modify(|_,w| w.mode().pull_up() );
}

#[cfg(feature = "board-nk3xn")]
type PwmTimer = lpc55_hal::peripherals::ctimer::Ctimer3<Unknown>;

pub fn init_rgb(syscon: &mut lpc55_hal::Syscon, iocon: &mut lpc55_hal::Iocon<Enabled>, ctimer: PwmTimer, clocks: &mut lpc55_hal::drivers::clocks::Clocks) -> board::led::RgbLed {
	#[cfg(any(feature = "board-lpcxpresso55"))]
	{ board::led::RgbLed::new(
		lpc55_hal::drivers::Pwm::new(ctimer.enabled(syscon, clocks.support_1mhz_fro_token().unwrap())),
		iocon,
	) }
	#[cfg(any(feature = "board-solo2", feature = "board-nk3xn"))]
	{ board::led::RgbLed::new(
		lpc55_hal::drivers::Pwm::new(ctimer.enabled(syscon, clocks.support_1mhz_fro_token().unwrap())),
		iocon,
	) }
}

#[cfg(feature = "usbfs-peripheral")]
pub type UsbPeripheralType = lpc55_hal::peripherals::usbfs::Usbfs;
#[cfg(not(feature = "usbfs-peripheral"))]
pub type UsbPeripheralType = lpc55_hal::peripherals::usbhs::Usbhs;

type UsbBusType = usb_device::bus::UsbBusAllocator<<types::Soc as crate::types::Soc>::UsbBus>;
type DelayTimer = Timer<lpc55_hal::peripherals::ctimer::Ctimer0<Enabled>>;

static mut USBD: Option<UsbBusType> = None;

pub fn setup_usb_bus(usbp: UsbPeripheralType, anactrl: &mut Anactrl, iocon: &mut Iocon<Enabled>, pmc: &mut Pmc, syscon: &mut Syscon, clocks: Clocks, delay_timer: &mut DelayTimer) -> &'static UsbBusType {
	let vbus_pin = pins::Pio0_22::take().unwrap().into_usb0_vbus_pin(iocon);

	let usb = usbp.enabled_as_device(
		anactrl,
		pmc,
		syscon,
		delay_timer,
		clocks.support_usbhs_token().unwrap(),
	);
	let usbd = lpc55_hal::drivers::UsbBus::new(usb, vbus_pin);

	unsafe { USBD.replace(usbd); }
	let usbd_ref = unsafe { USBD.as_ref().unwrap() };

	usbd_ref
}

pub fn setup_fm11nc08(
	clocks: &Clocks,
	syscon: &mut Syscon,
	iocon: &mut Iocon<Enabled>,
	gpio: &mut lpc55_hal::Gpio<Enabled>,
	flexcomm0: lpc55_hal::peripherals::flexcomm::Flexcomm0<Unknown>,
	inputmux: lpc55_hal::peripherals::inputmux::InputMux<Unknown>,
	pint: lpc55_hal::peripherals::pint::Pint<Unknown>,
	nfc_irq: lpc55_hal::Pin<nfc::NfcIrqPin, lpc55_hal::typestates::pin::state::Gpio<direction::Input>>,
	delay_timer: &mut DelayTimer
) -> Option<nfc::NfcChip> {
	let token = clocks.support_flexcomm_token().unwrap();
	let spi = flexcomm0.enabled_as_spi(syscon, &token);

	// TODO save these so they can be released later
	let mut mux = inputmux.enabled(syscon);
	let mut pint = pint.enabled(syscon);
	pint.enable_interrupt(&mut mux, &nfc_irq, lpc55_hal::peripherals::pint::Slot::Slot0, lpc55_hal::peripherals::pint::Mode::ActiveLow);
	mux.disabled(syscon);

	let force_nfc_reconfig = cfg!(feature = "reconfigure-nfc");

	nfc::try_setup(
		spi,
		gpio,
		iocon,
		nfc_irq,
		delay_timer,
		force_nfc_reconfig,
	)
}
