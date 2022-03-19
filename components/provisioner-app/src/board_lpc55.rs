use lpc55_hal;

pub fn uuid() -> [u8; 16] {
	lpc55_hal::uuid()
}

pub fn boot_to_bootrom() {
	use lpc55_hal::traits::flash::WriteErase;
	let flash = unsafe { lpc55_hal::peripherals::flash::Flash::steal() }.enabled(
		&mut unsafe { lpc55_hal::peripherals::syscon::Syscon::steal()}
	);
	lpc55_hal::drivers::flash::FlashGordon::new(flash).erase_page(0).ok();
	lpc55_hal::raw::SCB::sys_reset()
}
