use nrf52840_pac;

pub fn uuid() -> [u8; 16] {
	let mut uuid: [u8; 16] = [0; 16];
	let ficr = unsafe { nrf52840_pac::Peripherals::steal().FICR };
	uuid[0..4].copy_from_slice(&ficr.deviceid[0].read().bits().to_be_bytes());
	uuid[4..8].copy_from_slice(&ficr.deviceid[1].read().bits().to_be_bytes());
	uuid
}

pub fn boot_to_bootrom() {
}
