use littlefs2::const_ram_storage;
use nrf52840_hal::{
	gpio::{Pin, Input, Output, PushPull, PullDown, PullUp},
	spim,
	twim,
	uarte,
	usbd::{Usbd, UsbPeripheral},
};
use trussed::platform::{consent, reboot, ui};
use trussed::types::{LfsStorage, LfsResult};

//////////////////////////////////////////////////////////////////////////////
// Upper Interface (definitions towards ERL Core)

pub type FlashStorage = crate::soc::flash::FlashStorage;
const_ram_storage!(ExternalStorage, 8192);
/*
  I would love to use the real external flash here, but only if I find a way
  to hide the implementation details (= type parameters) from the type name
  of the upper interface. What if other SoCs access their flash chips through
  other busses - surely we don't want to accumulate a sh*tload of type
  parameters here? */
// pub type ExternalStorage = crate::soc::extflash::ExtFlashStorage<SPI, CS>;

/*
  The same rant as for ExternalStorage applies. However this time it's a
  lifetime issue and we can get away with forcing the RHS type to static,
  allowing us to drop the lifetime parameter from the LHS.
  See also src/types/usb.rs...
*/
pub type UsbBus = Usbd<UsbPeripheral<'static>>;

pub type Rng = chacha20::ChaCha8Rng;

pub const SYSCALL_IRQ: nrf52840_pac::Interrupt = nrf52840_pac::Interrupt::SWI0_EGU0;
pub static mut DEVICE_UUID: [u8; 16] = [0u8; 16];

pub fn device_uuid() -> &'static [u8; 16] { unsafe { &DEVICE_UUID } }

pub struct TrussedUI {
}

impl TrussedUI {
	pub fn new() -> Self { Self {} }
}

impl trussed::platform::UserInterface for TrussedUI {
	fn check_user_presence(&mut self) -> consent::Level {
		consent::Level::None
	}

	fn set_status(&mut self, _status: ui::Status) {
		info!("UI SetStatus");
	}

	fn refresh(&mut self) {}

	fn uptime(&mut self) -> core::time::Duration {
		// let _cyccnt = cortex_m::peripheral::DWT::get_cycle_count();
		core::time::Duration::new(0, 0)
	}

	fn reboot(&mut self, _to: reboot::To) -> ! {
		error!("TrussedUI::reboot() is deprecated!");
		panic!();
	}
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
