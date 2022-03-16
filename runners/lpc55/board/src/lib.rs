#![no_std]

pub use lpc55_hal as hal;

pub mod shared;
pub mod traits;

// board support package
#[cfg(not(any(feature = "lpcxpresso55", feature = "solo2")))]
compile_error!("Please select one of the board features.");

#[macro_use]
extern crate delog;
generate_macros!();

#[cfg(feature = "lpcxpresso55")]
pub mod lpcxpresso55;
#[cfg(feature = "lpcxpresso55")]
pub use lpcxpresso55 as specifics;

#[cfg(feature = "solo2")]
pub mod solo2;
#[cfg(feature = "solo2")]
pub use solo2 as specifics;

pub use shared::{
    Monotonic,
    Reboot,
};

pub use specifics::{
    button::ThreeButtons,
    led::RgbLed,
};

pub mod clock_controller;
pub mod nfc;
pub mod trussed;

// pub use rgb_led::RgbLed;
