use littlefs2::const_ram_storage;
use nrf52840_hal::{
	gpio::{Pin, Input, Output, PushPull, PullDown, PullUp},
	spim,
	twim,
	uarte,
	usbd::{Usbd, UsbPeripheral},
};
use trussed::types::{LfsStorage, LfsResult};

//////////////////////////////////////////////////////////////////////////////
// Upper Interface (definitions towards ERL Core)

pub static mut DEVICE_UUID: [u8; 16] = [0u8; 16];

const INTERFACE_CONFIG: crate::types::Config = crate::types::Config {
	card_issuer: b"Nitrokey/PTB\0",
	usb_product: super::board::USB_PRODUCT,
	usb_manufacturer: "Nitrokey/PTB",
	usb_serial: super::board::USB_SERIAL,
	usb_id_vendor: crate::types::USB_ID_VENDOR_NITROKEY,
	usb_id_product: super::board::USB_ID_PRODUCT,
};

/* the base address of the internal filesystem is compile-time configurable
   and placed into build_constants::CONFIG_FILESYSTEM_BOUNDARY */
pub const FILESYSTEM_END: usize = 0x000E_C000;

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
	type TrussedUI = super::dummy_ui::DummyUI;
	type Reboot = self::Reboot;

	const SYSCALL_IRQ: crate::types::IrqNr = crate::types::IrqNr { i: nrf52840_pac::Interrupt::SWI0_EGU0 as u16 };

	const SOC_NAME: &'static str = "NRF52840";
	const BOARD_NAME: &'static str = super::board::BOARD_NAME;
	const INTERFACE_CONFIG: &'static crate::types::Config = &INTERFACE_CONFIG;
	fn device_uuid() -> &'static [u8; 16] { unsafe { &DEVICE_UUID } }
}

pub struct DummyNfc;
impl nfc_device::traits::nfc::Device for DummyNfc {
	fn read(&mut self, buf: &mut [u8]) -> Result<nfc_device::traits::nfc::State, nfc_device::traits::nfc::Error> {
		Err(nfc_device::traits::nfc::Error::NoActivity)
	}
	fn send(&mut self, buf: &[u8]) -> Result<(), nfc_device::traits::nfc::Error> {
		Err(nfc_device::traits::nfc::Error::NoActivity)
	}
	fn frame_size(&self) -> usize { 0 }
}

pub struct Reboot {
}

#[cfg(feature = "admin-app")]
impl admin_app::Reboot for Reboot {
	fn reboot() -> ! { todo!() }
	fn reboot_to_firmware_update() -> ! { todo!() }
	fn reboot_to_firmware_update_destructive() -> ! { todo!() }
	fn locked() -> bool { todo!() }
}

//////////////////////////////////////////////////////////////////////////////
// Lower Interface (common definitions for individual boards)

pub struct BoardGPIO {
	/* interactive elements */
	pub buttons: [Option<Pin<Input<PullUp>>>; 8],
	pub leds: [Option<Pin<Output<PushPull>>>; 4],
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

	/* External Flash & NFC (through SPIM3) */
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
