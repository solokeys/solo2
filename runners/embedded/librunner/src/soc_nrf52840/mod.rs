pub mod types;

#[cfg_attr(feature = "board-nrfdk", path = "board_nrfdk.rs")]
pub mod board;

#[cfg(not(any(feature = "board-nrfdk")))]
compile_error!("No NRF52840 board chosen!");
