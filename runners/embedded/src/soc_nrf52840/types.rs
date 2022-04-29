use littlefs2::const_ram_storage;
use nrf52840_hal::{
	gpio::{Pin, Input, Output, PushPull, PullDown, PullUp},
	spim,
	twim,
	uarte,
	pac,
	usbd::{Usbd, UsbPeripheral},
};
use nrf52840_pac;

use trussed::types::{LfsStorage, LfsResult};


//////////////////////////////////////////////////////////////////////////////
// Upper Interface (definitions towards ERL Core)

pub static mut DEVICE_UUID: [u8; 16] = [0u8; 16];


const INTERFACE_CONFIG: crate::types::Config = crate::types::Config {
	card_issuer: &crate::types::build_constants::CCID_ISSUER,
	usb_product: crate::types::build_constants::USB_PRODUCT,
	usb_manufacturer: crate::types::build_constants::USB_MANUFACTURER,
	usb_serial:  "00000000-0000-0000-00000000",
	usb_id_vendor: crate::types::build_constants::USB_ID_VENDOR,
	usb_id_product: crate::types::build_constants::USB_ID_PRODUCT,
};

/* the base address of the internal filesystem is compile-time configurable
   and placed into build_constants::CONFIG_FILESYSTEM_END */
pub const FILESYSTEM_END: usize = crate::types::build_constants::CONFIG_FILESYSTEM_END;

#[cfg(not(feature = "extflash_qspi"))]
const_ram_storage!(ExternalStorage, 8192);

pub struct Soc {}
impl crate::types::Soc for Soc {
	type InternalFlashStorage = super::flash::FlashStorage;
	#[cfg(feature = "extflash_qspi")]
	type ExternalFlashStorage = super::qspiflash::QspiFlash;
	#[cfg(not(feature = "extflash_qspi"))]
	type ExternalFlashStorage = ExternalStorage;
	type UsbBus = Usbd<UsbPeripheral<'static>>;
	type NfcDevice = DummyNfc;
	type Rng = chacha20::ChaCha8Rng;
	type TrussedUI = super::board::TrussedUI;
	type Reboot = self::Reboot;

	type Duration = super::rtic_monotonic::RtcDuration;
	type Instant = super::rtic_monotonic::RtcInstant;

	const SYSCALL_IRQ: crate::types::IrqNr = crate::types::IrqNr { i: nrf52840_pac::Interrupt::SWI0_EGU0 as u16 };

	const SOC_NAME: &'static str = "NRF52840";
	const BOARD_NAME: &'static str = super::board::BOARD_NAME;
	const INTERFACE_CONFIG: &'static crate::types::Config = &INTERFACE_CONFIG;
	fn device_uuid() -> &'static [u8; 16] { unsafe { &DEVICE_UUID } }
}

pub struct DummyNfc;
impl nfc_device::traits::nfc::Device for DummyNfc {
	fn read(&mut self, _buf: &mut [u8]) -> Result<nfc_device::traits::nfc::State, nfc_device::traits::nfc::Error> {
		Err(nfc_device::traits::nfc::Error::NoActivity)
	}
	fn send(&mut self, _buf: &[u8]) -> Result<(), nfc_device::traits::nfc::Error> {
		Err(nfc_device::traits::nfc::Error::NoActivity)
	}
	fn frame_size(&self) -> usize { 0 }
}

pub struct Reboot {}


use crate::soc::types::pac::SCB;
use crate::soc::types::pac::power::GPREGRET;


#[cfg(feature = "admin-app")]
impl admin_app::Reboot for Reboot {
	fn reboot() -> ! {
		SCB::sys_reset()
	}
	fn reboot_to_firmware_update() -> ! {
		let pac = unsafe { nrf52840_pac::Peripherals::steal() };
		pac.POWER.gpregret.write(|w| unsafe { w.bits(0xb1 as u32) });

		SCB::sys_reset()
	}
	fn reboot_to_firmware_update_destructive() -> ! {
		// @TODO: come up with an idea how to
		// factory reset, and apply!
		SCB::sys_reset()
	}
	fn locked() -> bool {
		false
	}
}


//////////////////////////////////////////////////////////////////////////////
// Lower Interface (common definitions for individual boards)

pub struct BoardGPIO {
	/* interactive elements */
	pub buttons: [Option<Pin<Input<PullUp>>>; 8],
	pub leds: [Option<Pin<Output<PushPull>>>; 4],
	pub rgb_led: [Option<Pin<Output<PushPull>>>; 3],
	pub touch: Option<Pin<Output<PushPull>>>,

	/* UARTE0 */
	pub uart_pins: Option<uarte::Pins>,

	/* Fingerprint Reader (through UARTE0) */
	pub fpr_detect: Option<Pin<Input<PullDown>>>,
	pub fpr_power: Option<Pin<Output<PushPull>>>,

	/* LCD (through SPIM0) */
	pub display_spi: Option<spim::Pins>,
	pub display_cs: Option<Pin<Output<PushPull>>>,
	pub display_reset: Option<Pin<Output<PushPull>>>,
	pub display_dc: Option<Pin<Output<PushPull>>>,
	pub display_backlight: Option<Pin<Output<PushPull>>>,
	pub display_power: Option<Pin<Output<PushPull>>>,

	/* Secure Element (through TWIM1) */
	pub se_pins: Option<twim::Pins>,
	pub se_power: Option<Pin<Output<PushPull>>>,

	/* External Flash & NFC (through SxPIM3) */
	pub flashnfc_spi: Option<spim::Pins>,
	pub flash_cs: Option<Pin<Output<PushPull>>>,
	pub flash_power: Option<Pin<Output<PushPull>>>,
	pub nfc_cs: Option<Pin<Output<PushPull>>>,
	pub nfc_irq: Option<Pin<Input<PullUp>>>,
}

pub fn is_pin_latched<T>(pin: &Pin<Input<T>>, latches: &[u32]) -> bool {
	let pinport = match pin.port() {
		nrf52840_hal::gpio::Port::Port0 => 0,
		nrf52840_hal::gpio::Port::Port1 => 1
	};
	let pinshift = pin.pin();

	((latches[pinport] >> pinshift) & 1) != 0
}

pub fn is_keepalive_pin(pinport: u32) -> bool {
	super::board::KEEPALIVE_PINS.contains(&(pinport as u8))
}
