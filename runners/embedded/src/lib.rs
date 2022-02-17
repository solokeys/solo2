#![no_std]

use littlefs2::fs::Filesystem;

#[macro_use]
extern crate delog;
delog::generate_macros!();

pub mod types;

#[cfg(not(any(feature = "soc-lpc55", feature = "soc-nrf52840")))]
compile_error!("No SoC chosen!");

#[cfg_attr(feature = "soc-nrf52840", path = "soc_nrf52840/mod.rs")]
#[cfg_attr(feature = "soc-lpc55", path = "soc_lpc55/mod.rs")]
pub mod soc;

pub fn init_store(int_flash: soc::types::FlashStorage, ext_flash: soc::types::ExternalStorage) -> types::RunnerStore {
	unsafe {
		types::INTERNAL_STORAGE = Some(int_flash);
		types::EXTERNAL_STORAGE = Some(ext_flash);
		types::VOLATILE_STORAGE = Some(types::VolatileStorage::new());

		types::INTERNAL_FS_ALLOC = Some(Filesystem::allocate());
		types::EXTERNAL_FS_ALLOC = Some(Filesystem::allocate());
		types::VOLATILE_FS_ALLOC = Some(Filesystem::allocate());
	}

	let store = types::RunnerStore::claim().unwrap();

	store.mount(
		unsafe { types::INTERNAL_FS_ALLOC.as_mut().unwrap() },
		unsafe { types::INTERNAL_STORAGE.as_mut().unwrap() },
		unsafe { types::EXTERNAL_FS_ALLOC.as_mut().unwrap() },
		unsafe { types::EXTERNAL_STORAGE.as_mut().unwrap() },
		unsafe { types::VOLATILE_FS_ALLOC.as_mut().unwrap() },
		unsafe { types::VOLATILE_STORAGE.as_mut().unwrap() },
		true
        ).expect("store.mount() error");

	store
}
