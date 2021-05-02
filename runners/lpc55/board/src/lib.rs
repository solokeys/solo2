#![no_std]

pub use lpc55_hal as hal;

pub mod traits;

// board support package
#[cfg(not(any(feature = "board-lpcxpresso55", feature = "board-solo2")))]
compile_error!("Please select one of the board features.");

#[cfg(feature = "board-lpcxpresso55")]
pub mod lpcxpresso55;
#[cfg(feature = "board-lpcxpresso55")]
pub use lpcxpresso55 as specifics;

#[macro_use]
extern crate delog;
generate_macros!();

#[cfg(feature = "board-solo2")]
pub mod solo2;
#[cfg(feature = "board-solo2")]
pub use solo2 as specifics;

pub use specifics::{
    button::ThreeButtons,
    led::RgbLed,
};

pub mod clock_controller;
pub mod nfc;
pub mod trussed;

// pub use rgb_led::RgbLed;
