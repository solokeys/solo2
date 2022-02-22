// use littlefs2::const_ram_storage;
use nrf52840_hal::{
	gpio::{Pin, Input, Output, PushPull, PullDown, PullUp},
	spim,
	twim,
	uarte,
	usbd::{Usbd, UsbPeripheral},
};
// use trussed::types::{LfsStorage, LfsResult};

//////////////////////////////////////////////////////////////////////////////
// Upper Interface (definitions towards ERL Core)

pub static mut DEVICE_UUID: [u8; 16] = [0u8; 16];

pub struct Soc {}
impl crate::types::Soc for Soc {
	type InternalFlashStorage = super::flash::FlashStorage;
/* If the choice of SPIM ever differs between products, change the first
   type parameter to crate::soc::board::SOMETHING and handle it further down */
	type ExternalFlashStorage = super::extflash::ExtFlashStorage<
		nrf52840_hal::spim::Spim<nrf52840_pac::SPIM3>,
		Pin<Output<PushPull>>>;
	type UsbBus = Usbd<UsbPeripheral<'static>>;
	type Rng = chacha20::ChaCha8Rng;
	type TrussedUI = super::dummy_ui::DummyUI;
	type Reboot = self::Reboot;

	const SOC_NAME: &'static str = "NRF52840";
	const BOARD_NAME: &'static str = super::board::BOARD_NAME;
	const SYSCALL_IRQ: crate::types::IrqNr = crate::types::IrqNr { i: nrf52840_pac::Interrupt::SWI0_EGU0 as u16 };

	fn device_uuid() -> &'static [u8; 16] { unsafe { &DEVICE_UUID } }
}

pub struct Reboot {
}

#[cfg(feature = "admin-app")]
impl admin_app::Reboot for Reboot {
	fn reboot() -> ! { todo!() }
	fn reboot_to_firmware_update() -> ! { todo!() }
	fn reboot_to_firmware_update_destructive() -> ! { todo!() }
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
