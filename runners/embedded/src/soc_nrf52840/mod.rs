pub mod types;

#[cfg(not(any(feature = "board-nrfdk")))]
compile_error!("No NRF52840 board chosen!");

#[cfg_attr(feature = "board-nrfdk", path = "board_nrfdk.rs")]
pub mod board;
