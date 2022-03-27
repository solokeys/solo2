// use crate::spi_nor_flash::SpiNorFlash;
use embedded_hal::blocking::{
	delay::DelayMs,
	spi::Transfer
};
use nrf52840_hal::{
	gpio::{Output, Pin, PushPull},
	prelude::OutputPin,
	// spim::TransferSplitRead,
};
use spi_memory::{BlockDevice, Read};

struct FlashProperties {
	size: usize,
	jedec: [u8; 3],
	_cont: u8,
}

/* GD25Q16C, 16 Mbit == 2 MB */
#[cfg(any(feature = "board-proto1", feature = "board-nk3am"))]
const FLASH_GD25Q16C: FlashProperties = FlashProperties {
	size: 0x20_0000,
	jedec: [0xc8, 0x40, 0x15],
	_cont: 0 /* should be 6, but device doesn't report those */
};
#[cfg(any(feature = "board-proto1", feature = "board-nk3am"))]
const FLASH_PROPERTIES: &FlashProperties = &FLASH_GD25Q16C;

/* MX25R6435F, 64 Mbit == 8 MB */
#[cfg(feature = "board-nrfdk")]
const FLASH_MX25R6435F: FlashProperties = FlashProperties {
	size: 0x80_0000,
	jedec: [0xc2, 0x28, 0x17],
	_cont: 0
};
#[cfg(feature = "board-nrfdk")]
const FLASH_PROPERTIES: &FlashProperties = &FLASH_MX25R6435F;

pub struct ExtFlashStorage<SPI, CS> where SPI: Transfer<u8>, CS: OutputPin {
	s25flash: spi_memory::series25::Flash<SPI, CS>,
	power_pin: Option<Pin<Output<PushPull>>>,
}

impl<SPI, CS> littlefs2::driver::Storage for ExtFlashStorage<SPI, CS> where SPI: Transfer<u8>, CS: OutputPin {

	const BLOCK_SIZE: usize = 4096;
	const READ_SIZE: usize = 4;
	const WRITE_SIZE: usize = 256;
	const BLOCK_COUNT: usize = FLASH_PROPERTIES.size / Self::BLOCK_SIZE;
	type CACHE_SIZE = generic_array::typenum::U256;
	type LOOKAHEADWORDS_SIZE = generic_array::typenum::U1;

	fn read(&mut self, off: usize, buf: &mut [u8]) -> Result<usize, littlefs2::io::Error> {
		trace!("EFr {:x} {:x}", off, buf.len());
		if buf.len() == 0 { return Ok(0); }
		if buf.len() > FLASH_PROPERTIES.size ||
			off > FLASH_PROPERTIES.size - buf.len() {
			return Err(littlefs2::io::Error::Unknown(0x6578_7046));
		}
		let r = self.s25flash.read(off as u32, buf);
		if r.is_ok() { trace!("r >>> {}", delog::hex_str!(&buf[0..4])); }
		map_result(r, buf.len())
	}

	/* Holy sh*t, could these moronic Trait designers finally make up their mind
		about the mutability of their function arguments... why the f**k is
		spi_memory::BlockDevice expecting a mutable buffer on write_bytes()!? */
	fn write(&mut self, off: usize, buf: &[u8]) -> Result<usize, littlefs2::io::Error> {
		trace!("EFw {:x} {:x}", off, buf.len());
		trace!("w >>> {}", delog::hex_str!(&buf[0..4]));
/*
		let mut i: usize = 0;
		while i < buf.len() {
			let ilen: usize = core::cmp::min(buf.len() - i, 256);
			let r = self._write(off + i, &buf[i..i+ilen]);
			if r.is_err() {
				return r;
			}
			i += ilen;
		}
		Ok(buf.len())
*/
		let r = self.s25flash.write_bytes(off as u32, buf);
		map_result(r, buf.len())
	}

	fn erase(&mut self, off: usize, len: usize) -> Result<usize, littlefs2::io::Error> {
		trace!("EFe {:x} {:x}", off, len);
		if len > FLASH_PROPERTIES.size ||
			off > FLASH_PROPERTIES.size - len {
			return Err(littlefs2::io::Error::Unknown(0x6578_7046));
		}
		map_result(self.s25flash.erase_sectors(off as u32, len), len)
	}
}

fn map_result<SPI, CS>(r: Result<(), spi_memory::Error<SPI, CS>>, len: usize)
			-> Result<usize, littlefs2::io::Error>
			where SPI: Transfer<u8>, CS: OutputPin {
	match r {
		Ok(()) => Ok(len),
		Err(_) => Err(littlefs2::io::Error::Unknown(0x6565_6565))
	}
}

impl<SPI, CS> ExtFlashStorage<SPI, CS> where SPI: Transfer<u8>, CS: OutputPin {

	fn raw_command(spim: &mut SPI, cs: &mut CS, buf: &mut [u8]) {
		cs.set_low().ok();
		spim.transfer(buf);
		cs.set_high().ok();
	}

	pub fn new(mut spim: SPI, mut cs: CS, mut power_pin: Option<Pin<Output<PushPull>>>, delay_timer: &mut dyn DelayMs<u32>) -> Self {
		if let Some(p) = power_pin.as_mut() {
			p.set_high().ok();
			delay_timer.delay_ms(200u32);
		}

		Self::selftest(&mut spim, &mut cs);

		let mut flash = spi_memory::series25::Flash::init(spim, cs).ok().unwrap();
		let jedec_id = flash.read_jedec_id().ok().unwrap();
		info!("NRF Ext. Flash: {:?}", jedec_id);
		if jedec_id.mfr_code() != FLASH_PROPERTIES.jedec[0] ||
			jedec_id.device_id() != &FLASH_PROPERTIES.jedec[1..] {
			panic!("Unknown Ext. Flash!");
		}

		Self { s25flash: flash, power_pin }
	}

	pub fn selftest(spim: &mut SPI, cs: &mut CS) {
		macro_rules! doraw {
			($buf:expr, $len:expr, $str:expr) => {
			let mut buf: [u8; $len] = $buf;
			Self::raw_command(spim, cs, &mut buf);
			trace!($str, delog::hex_str!(&buf[1..]));
		}}

		doraw!([0x9f, 0, 0, 0], 4, "JEDEC {}");
		doraw!([0x05, 0], 2, "RDSRl {}");
		doraw!([0x35, 0], 2, "RDSRh {}");
	}

	pub fn size(&self) -> usize {
		FLASH_PROPERTIES.size
	}

	pub fn erase_chip(&mut self) -> Result<usize, littlefs2::io::Error> {
		map_result(self.s25flash.erase_all(), FLASH_PROPERTIES.size)
	}

	pub fn power_on(&mut self, delay_timer: &mut dyn DelayMs<u32>) {
		if let Some(pwr_pin) = self.power_pin.as_mut() {
			pwr_pin.set_high().ok();
			delay_timer.delay_ms(200u32);
		}
	}

	pub fn power_off(&mut self) {
		if let Some(pwr_pin) = self.power_pin.as_mut() {
			pwr_pin.set_low().ok();
		}
	}

	fn _write(&mut self, off: usize, buf: &[u8]) -> Result<usize, littlefs2::io::Error> {
		if buf.len() > FLASH_PROPERTIES.size ||
			off > FLASH_PROPERTIES.size - buf.len() {
			return Err(littlefs2::io::Error::Unknown(0x6578_7046));
		}
		let mut buf2: [u8; 256] = [0u8; 256];
		buf2[0..buf.len()].copy_from_slice(buf);
		map_result(self.s25flash.write_bytes(off as u32, &mut buf2), buf.len())
	}
}
