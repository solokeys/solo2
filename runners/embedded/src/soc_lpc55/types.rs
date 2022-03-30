use crate::types::build_constants;
use littlefs2::const_ram_storage;
use lpc55_hal::{
	drivers::timer,
	peripherals::{ctimer, flash, syscon, rng},
	raw,
	traits::flash::WriteErase,
};
use super::trussed::UserInterface;
use super::board::{button::ThreeButtons, led::RgbLed};
use trussed::types::{LfsResult, LfsStorage};

//////////////////////////////////////////////////////////////////////////////
// Upper Interface (definitions towards ERL Core)

pub static mut DEVICE_UUID: [u8; 16] = [0u8; 16];

const_ram_storage!(ExternalRAMStorage, 1024);

#[cfg(feature = "no-encrypted-storage")]
use lpc55_hal::littlefs2_filesystem;
#[cfg(not(feature = "no-encrypted-storage"))]
use lpc55_hal::littlefs2_prince_filesystem;

#[cfg(feature = "no-encrypted-storage")]
littlefs2_filesystem!(InternalFilesystem: (build_constants::CONFIG_FILESYSTEM_BOUNDARY));
#[cfg(not(feature = "no-encrypted-storage"))]
littlefs2_prince_filesystem!(InternalFilesystem: (build_constants::CONFIG_FILESYSTEM_BOUNDARY));

#[cfg(feature = "usbfs-peripheral")]
type UsbPeripheral = lpc55_hal::peripherals::usbfs::EnabledUsbfsDevice;
#[cfg(not(feature = "usbfs-peripheral"))]
type UsbPeripheral = lpc55_hal::peripherals::usbhs::EnabledUsbhsDevice;

const INTERFACE_CONFIG: crate::types::Config = crate::types::Config {
	card_issuer: b"Nitrokey 3\0\0\0",
	usb_product: "--from PFR!--", /* TODO */
	usb_manufacturer: "Nitrokey",
	usb_serial: "00000000-0000-0000-00000000",
	usb_id_vendor: crate::types::USB_ID_VENDOR_NITROKEY,
	usb_id_product: 0x42b2_u16,
};

pub struct Soc {}
impl crate::types::Soc for Soc {
	type InternalFlashStorage = InternalFilesystem;
	type ExternalFlashStorage = ExternalRAMStorage;
	type UsbBus = lpc55_hal::drivers::UsbBus<UsbPeripheral>;
	type NfcDevice = super::nfc::NfcChip;
	type Rng = rng::Rng<lpc55_hal::Enabled>;
	type TrussedUI = UserInterface<ThreeButtons, RgbLed>;
	type Reboot = Lpc55Reboot;

	type Instant = ();
	type Duration = LpcTimerDuration;

	const SYSCALL_IRQ: crate::types::IrqNr = crate::types::IrqNr { i: raw::Interrupt::OS_EVENT as u16 };

	const SOC_NAME: &'static str = "LPC55";
	const BOARD_NAME: &'static str = super::board::BOARD_NAME;
	const INTERFACE_CONFIG: &'static crate::types::Config = &INTERFACE_CONFIG;
	fn device_uuid() -> &'static [u8; 16] { unsafe { &DEVICE_UUID } }
}

pub struct LpcTimerDuration { /* TODO: code me */ }
impl From<embedded_time::duration::units::Milliseconds> for LpcTimerDuration {
	fn from(ms: embedded_time::duration::units::Milliseconds) -> Self { Self{} }
}

pub struct Lpc55Reboot {}
impl admin_app::Reboot for Lpc55Reboot {
	fn reboot() -> ! {
		raw::SCB::sys_reset()
	}
	fn reboot_to_firmware_update() -> ! {
		lpc55_hal::boot_to_bootrom()
	}
	fn reboot_to_firmware_update_destructive() -> ! {
		// Erasing the first flash page & rebooting will keep processor in bootrom persistently.
		// This is however destructive, as a valid firmware will need to be flashed.
		let flash = unsafe { flash::Flash::steal() }.enabled(
			&mut unsafe { syscon::Syscon::steal() }
		);
		lpc55_hal::drivers::flash::FlashGordon::new(flash).erase_page(0).ok();
		raw::SCB::sys_reset()
	}
	fn locked() -> bool { todo!() }
}

pub type DynamicClockController = super::clock_controller::DynamicClockController;
pub type NfcWaitExtender = timer::Timer<ctimer::Ctimer0<lpc55_hal::typestates::init_state::Enabled>>;
pub type PerformanceTimer = timer::Timer<ctimer::Ctimer4<lpc55_hal::typestates::init_state::Enabled>>;
