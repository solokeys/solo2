use embedded_hal::blocking::spi::Transfer;
use nrf52840_hal::{
	clocks::Clocks,
	gpio::{Pin, Output, PushPull},
	prelude::OutputPin
};

pub mod types;

#[cfg(not(any(feature = "board-nrfdk", feature = "board-proto1", feature = "board-nk3am")))]
compile_error!("No NRF52840 board chosen!");

#[cfg_attr(feature = "board-nrfdk", path = "board_nrfdk.rs")]
#[cfg_attr(feature = "board-proto1", path = "board_proto1.rs")]
#[cfg_attr(feature = "board-nk3am", path = "board_nk3am.rs")]
pub mod board;

pub mod dummy_ui;
mod extflash;
mod flash;

pub fn init_bootup(ficr: &nrf52840_pac::FICR, uicr: &nrf52840_pac::UICR, power: &mut nrf52840_pac::POWER) {
	let deviceid0 = ficr.deviceid[0].read().bits();
	let deviceid1 = ficr.deviceid[1].read().bits();
	unsafe {
		types::DEVICE_UUID[0..4].copy_from_slice(&deviceid0.to_be_bytes());
		types::DEVICE_UUID[4..8].copy_from_slice(&deviceid1.to_be_bytes());
	}

	info!("RESET Reason: {:x}", power.resetreas.read().bits());
	power.resetreas.write(|w| w);

	info!("FICR DeviceID {}", delog::hex_str!(unsafe { &types::DEVICE_UUID[0..8] }));
	info!("FICR IdtRoot  {:08x} {:08x} {:08x} {:08x}",
		ficr.ir[0].read().bits(), ficr.ir[1].read().bits(),
		ficr.ir[2].read().bits(), ficr.ir[3].read().bits());
	info!("FICR EncRoot  {:08x} {:08x} {:08x} {:08x}",
		ficr.er[0].read().bits(), ficr.er[1].read().bits(),
		ficr.er[2].read().bits(), ficr.er[3].read().bits());
	let mut deviceaddr: [u8; 6] = [0u8; 6];
	deviceaddr[2..6].copy_from_slice(&ficr.deviceaddr[0].read().bits().to_be_bytes());
	deviceaddr[0..2].copy_from_slice(&(ficr.deviceaddr[1].read().bits() as u16).to_be_bytes());
	info!("FICR DevAddr  {}", delog::hex_str!(&deviceaddr));

	info!("UICR REGOUT0 {:x} NFCPINS {:x}",
		uicr.regout0.read().bits(), uicr.nfcpins.read().bits());
	if !uicr.regout0.read().vout().is_3v3() {
		error_now!("REGOUT0 is not at 3.3V - external flash will fail!");
	}
}

pub fn init_internal_flash(nvmc: nrf52840_pac::NVMC) -> flash::FlashStorage {
	flash::FlashStorage::new(nvmc)
}

pub fn init_external_flash<SPI, CS>(spim: SPI, cs: CS,
		pwr: Option<Pin<Output<PushPull>>>)
		-> extflash::ExtFlashStorage<SPI, CS> where SPI: Transfer<u8>, CS: OutputPin {
	extflash::ExtFlashStorage::new(spim, cs, pwr)
}

type UsbClockType = Clocks<nrf52840_hal::clocks::ExternalOscillator, nrf52840_hal::clocks::Internal, nrf52840_hal::clocks::LfOscStarted>;
type UsbBusType = usb_device::bus::UsbBusAllocator<<types::Soc as crate::types::Soc>::UsbBus>;

static mut USB_CLOCK: Option<UsbClockType> = None;
static mut USBD: Option<UsbBusType> = None;

pub fn setup_usb_bus(clock: nrf52840_pac::CLOCK, usbd: nrf52840_pac::USBD) -> &'static UsbBusType {
	let usb_clock = Clocks::new(clock).start_lfclk().enable_ext_hfosc();
	unsafe { USB_CLOCK.replace(usb_clock); }
	let usb_clock_ref = unsafe { USB_CLOCK.as_ref().unwrap() };

	let usb_peripheral = nrf52840_hal::usbd::UsbPeripheral::new(usbd, usb_clock_ref);

	let usbd = nrf52840_hal::usbd::Usbd::new(usb_peripheral);
	unsafe { USBD.replace(usbd); }
	let usbd_ref = unsafe { USBD.as_ref().unwrap() };

	usbd_ref
}
