// use crate::spi_nor_flash::SpiNorFlash;
use embedded_hal::blocking::spi::Transfer;
use nrf52840_hal::{
	gpio::{Output, Pin, PushPull},
	prelude::OutputPin,
	// spim::TransferSplitRead,
};

struct FlashProperties {
	flash_size: usize,
	flash_jedec: [u8; 12],
}

#[cfg(feature = "board-proto1")]
const FLASH_PROPERTIES: FlashProperties = FlashProperties {
	/* GD25Q16C, 16 Mbit == 2 MB */
	flash_size: 0x20_0000,
	/* should really be [0x00, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0xc8, 0x40, 0x15, 0xc8],
	   but GigaDevice doesn't understand JEDEC216 */
	flash_jedec: [0x00, 0xc8, 0x40, 0x15, 0xc8, 0x40, 0x15, 0xc8, 0x40, 0x15, 0xc8, 0x40],
};

#[cfg(feature = "board-nk3mini")]
const FLASH_PROPERTIES: FlashProperties = FlashProperties {
	/* GD25Q16C, 16 Mbit == 2 MB */
	flash_size: 0x20_0000,
	/* should really be [0x00, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0xc8, 0x40, 0x15, 0xc8],
	   but GigaDevice doesn't understand JEDEC216 */
	flash_jedec: [0x00, 0xc8, 0x40, 0x15, 0xc8, 0x40, 0x15, 0xc8, 0x40, 0x15, 0xc8, 0x40],
};

#[cfg(feature = "board-nrfdk")]
const FLASH_PROPERTIES: FlashProperties = FlashProperties {
	/* MX25R6435F, 64 Mbit == 8 MB */
	flash_size: 0x80_0000,
	flash_jedec: [0x00, 0xc2, 0x28, 0x17, 0xc2, 0x28, 0x17, 0xc2, 0x28, 0x17, 0xc2, 0x28],
};

pub struct ExtFlashStorage<SPI, CS> where SPI: Transfer<u8>, CS: OutputPin {
	s25flash: spi_memory::series25::Flash<SPI, CS>,
	power_pin: Option<Pin<Output<PushPull>>>,
}

impl<SPI, CS> littlefs2::driver::Storage for ExtFlashStorage<SPI, CS> where SPI: Transfer<u8>, CS: OutputPin {

	const BLOCK_SIZE: usize = 4096;
	const READ_SIZE: usize = 4;
	const WRITE_SIZE: usize = 4;
	const BLOCK_COUNT: usize = FLASH_PROPERTIES.flash_size / Self::BLOCK_SIZE;
	type CACHE_SIZE = generic_array::typenum::U256;
	type LOOKAHEADWORDS_SIZE = generic_array::typenum::U1;

	fn read(&self, off: usize, buf: &mut [u8]) -> Result<usize, littlefs2::io::Error> {
		if off + buf.len() > FLASH_PROPERTIES.flash_size {
			return Err(littlefs2::io::Error::Unknown(0x6578_7046));
		}
		// self.spim.transfer()
		// let _buf: [u8; 4] = [0x03, (off >> 16) as u8, (off >> 8) as u8, off as u8];

		trace!("F RD {:x} {:x}", off, buf.len());
		Err(littlefs2::io::Error::Unknown(0x6565_6565))
	}

	fn write(&mut self, off: usize, buf: &[u8]) -> Result<usize, littlefs2::io::Error> {
		trace!("F WR {:x} {:x}", off, buf.len());
		Err(littlefs2::io::Error::Unknown(0x6565_6565))
	}

	fn erase(&mut self, off: usize, len: usize) -> Result<usize, littlefs2::io::Error> {
		trace!("F ER {:x} {:x}", off, len);
		Err(littlefs2::io::Error::Unknown(0x6565_6565))
	}
}

impl<SPI, CS> ExtFlashStorage<SPI, CS> where SPI: Transfer<u8>, CS: OutputPin {

	pub fn new(spim: SPI, cs: CS, mut power_pin: Option<Pin<Output<PushPull>>>) -> Self {
		if let Some(p) = power_pin.as_mut() {
			p.set_high().ok();
		}

		let mut flash = spi_memory::series25::Flash::init(spim, cs).ok().unwrap();
		let jedec = flash.read_jedec_id().ok().unwrap();
		info!("NRF Ext. Flash: {:?}", jedec);

		Self { s25flash: flash, power_pin }
	}

	fn power_on(&mut self) {
		if let Some(pwr_pin) = self.power_pin.as_mut() {
			pwr_pin.set_high().ok();
			// TODO: crate::board_delay(200u32);
		}
	}

	pub fn power_off(&mut self) {
		if let Some(pwr_pin) = self.power_pin.as_mut() {
			pwr_pin.set_low().ok();
		}
	}

}
