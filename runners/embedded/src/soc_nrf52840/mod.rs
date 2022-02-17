use embedded_hal::blocking::spi::Transfer;
use nrf52840_hal::gpio::{
	Pin, Output, PushPull
};
use nrf52840_hal::prelude::OutputPin;

pub mod types;

#[cfg(not(any(feature = "board-nrfdk")))]
compile_error!("No NRF52840 board chosen!");

#[cfg_attr(feature = "board-nrfdk", path = "board_nrfdk.rs")]
#[cfg_attr(feature = "board-proto1", path = "board_proto1.rs")]
#[cfg_attr(feature = "board-nk3mini", path = "board_nk3mini.rs")]
pub mod board;

mod extflash;
mod flash;

pub fn init_internal_flash(nvmc: nrf52840_pac::NVMC) -> flash::FlashStorage {
	flash::FlashStorage::new(nvmc)
}

pub fn init_external_flash<SPI, CS>(spim: SPI, cs: CS,
		pwr: Option<Pin<Output<PushPull>>>)
		-> extflash::ExtFlashStorage<SPI, CS> where SPI: Transfer<u8>, CS: OutputPin {
	extflash::ExtFlashStorage::new(spim, cs, pwr)
}
