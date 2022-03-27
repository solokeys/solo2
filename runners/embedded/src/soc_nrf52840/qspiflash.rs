use embedded_hal::blocking::delay::DelayMs;
use nrf52840_hal::gpio::{Input, Floating, Output, Pin, PushPull};
use nrf52840_hal::spim::Pins as SpimPins;
use nrf52840_hal::prelude::OutputPin;

pub struct QspiFlash {
	qspi: nrf52840_pac::QSPI,
	clk_pin: Pin<Output<PushPull>>,
	cs_pin: Pin<Output<PushPull>>,
	io0_pin: Pin<Output<PushPull>>,
	io1_pin: Pin<Input<Floating>>,

	power_pin: Option<Pin<Output<PushPull>>>,
}

impl QspiFlash {
	/* GD25Q16C, 16 Mbit == 2 MB, c8 40 15 */
	#[cfg(any(feature = "board-nk3am", feature = "board-proto1"))]
	const FLASH_SIZE: usize = 0x20_0000;
	/* MX25R6435F, 64 Mbit == 8 MB, c2 28 17 */
	#[cfg(feature = "board-nrfdk")]
	const FLASH_SIZE: usize = 0x80_0000;

	pub fn new(qspi_pac: nrf52840_pac::QSPI,
		mut spi_pins: SpimPins,
		cs_pin: Pin<Output<PushPull>>,
		power_pin: Option<Pin<Output<PushPull>>>,
		delay_timer: &mut dyn DelayMs<u32>) -> Self {
		let mut obj = Self {
			qspi: qspi_pac,
			clk_pin: spi_pins.sck,
			cs_pin,
			io0_pin: spi_pins.mosi.take().unwrap(),
			io1_pin: spi_pins.miso.take().unwrap(),
			power_pin
		};

		if obj.power_pin.is_some() {
			obj.power_pin.as_mut().unwrap().set_high().ok();
			delay_timer.delay_ms(200u32);
		}

		obj.qspi.psel.sck.write(|w| unsafe { w.bits(obj.clk_pin.psel_bits()) });
		obj.qspi.psel.csn.write(|w| unsafe { w.bits(obj.cs_pin.psel_bits()) });
		obj.qspi.psel.io0.write(|w| unsafe { w.bits(obj.io0_pin.psel_bits()) });
		obj.qspi.psel.io1.write(|w| unsafe { w.bits(obj.io1_pin.psel_bits()) });
                obj.qspi.ifconfig0.write(|w| w.readoc().fastread()
					.writeoc().pp()
					.addrmode()._24bit()
					.dpmenable().disable()
					.ppsize()._256bytes() );
                obj.qspi.ifconfig1.write(|w| unsafe { w
					.sckfreq().bits(15)
					.spimode().mode0()
					.sckdelay().bits(2) });

		obj
	}

	pub fn activate(&mut self) {
                self.qspi.enable.write(|w| w.enable().enabled() );
                self.qspi.tasks_activate.write(|w| w.tasks_activate().set_bit() );
		self.wait_ready();
	}

	pub fn wait_ready(&self) {
		while !self.qspi.events_ready.read().events_ready().bits() {}
		self.qspi.events_ready.write(|w| unsafe { w.bits(0) });
	}

	pub fn wait_wip_clear(&self) {
		while self.qspi.status.read().sreg().bits() & 1 != 0 {}
	}

	pub fn read_jedec_id(&mut self) -> [u8; 3] {
		self.qspi.cinstrdat0.write(|w| unsafe { w.bits(0) });
		self.qspi.cinstrconf.write(|w| unsafe { w.opcode().bits(0x9F)
					.length()._4b()
					.wipwait().clear_bit()
					.wren().clear_bit()
					.lfen().clear_bit()
					.lfstop().clear_bit() });
		self.wait_ready();
		let val = self.qspi.cinstrdat0.read().bits();

		[val as u8, (val >> 8) as u8, (val >> 16) as u8]
	}

	pub fn _erase(&mut self, off: usize) {
		self.qspi.erase.ptr.write(|w| unsafe { w.bits(off as u32) });

		self.qspi.tasks_erasestart.write(|w| w.tasks_erasestart().set_bit() );
		self.wait_ready();
	}
}

impl littlefs2::driver::Storage for QspiFlash {

        const BLOCK_SIZE: usize = 0x1000;
        const READ_SIZE: usize = 4;
        const WRITE_SIZE: usize = 256;
        const BLOCK_COUNT: usize = Self::FLASH_SIZE / Self::BLOCK_SIZE;
        type CACHE_SIZE = generic_array::typenum::U256;
        type LOOKAHEADWORDS_SIZE = generic_array::typenum::U1;

        fn read(&mut self, off: usize, buf: &mut [u8]) -> Result<usize, littlefs2::io::Error> {
		trace!("EFr {:x} {:x}", off, buf.len());
		let bufptr: *mut u8 = buf.as_mut_ptr();
		if (bufptr as usize & buf.len() & (Self::READ_SIZE - 1)) != 0 {
			return Err(littlefs2::io::Error::Invalid);
		}
		self.qspi.read.src.write(|w| unsafe { w.bits(off as u32) });
		self.qspi.read.dst.write(|w| unsafe { w.bits(bufptr as u32) });
		self.qspi.read.cnt.write(|w| unsafe { w.bits(buf.len() as u32) });
		core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

		self.qspi.tasks_readstart.write(|w| w.tasks_readstart().set_bit() );
		self.wait_ready();

		core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
		Ok(buf.len())
	}

	fn write(&mut self, off: usize, buf: &[u8]) -> Result<usize, littlefs2::io::Error> {
		trace!("EFw {:x} {:x}", off, buf.len());
		let bufptr: *const u8 = buf.as_ptr();
		if (bufptr as usize & buf.len() & (Self::READ_SIZE - 1)) != 0 {
			return Err(littlefs2::io::Error::Invalid);
		}
		self.qspi.write.dst.write(|w| unsafe { w.bits(off as u32) });
		self.qspi.write.src.write(|w| unsafe { w.bits(bufptr as u32) });
		self.qspi.write.cnt.write(|w| unsafe { w.bits(buf.len() as u32) });
		core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

		self.qspi.tasks_writestart.write(|w| w.tasks_writestart().set_bit() );
		self.wait_ready();

		core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
		Ok(buf.len())
	}

	fn erase(&mut self, off: usize, len: usize) -> Result<usize, littlefs2::io::Error> {
		trace!("EFe {:x} {:x}", off, len);
		let mut step: usize;
		if off == 0 && len == Self::FLASH_SIZE {
			self.qspi.erase.len.write(|w| w.len().all());
			self._erase(0);
			return Ok(len);
		} else if (off & len & (0x1_0000 - 1)) == 0 {
			self.qspi.erase.len.write(|w| w.len()._64kb());
			step = 0x1_0000;
		} else if (off & len & (0x1000 - 1)) == 0 {
			self.qspi.erase.len.write(|w| w.len()._4kb());
			step = 0x1000;
		} else {
			return Err(littlefs2::io::Error::Invalid);
		}

		for x in (off..off+len).step_by(step) {
			self._erase(x);
		}
		return Ok(len);
	}
}
