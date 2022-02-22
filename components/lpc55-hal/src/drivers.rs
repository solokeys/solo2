//! Drivers for device functionality.
//!
//! Typically, these drivers take ownership of one or more HAL peripherals,
//! and expose functionality defined in a separate trait.

pub mod prelude {
    pub use super::i2c::prelude::*;
    pub use super::spi::prelude::*;
    pub use super::usbd::prelude::*;
}

pub mod aes;
pub use aes::{Aes, Key as AesKey};

pub mod clocks;
pub use clocks::ClockRequirements;

pub mod pins;
pub use pins::{
    Pin,
    Pins,
};

pub mod flash;
pub use flash::FlashGordon;

pub mod gint;
pub use gint::GroupInterrupt;

pub mod i2c;
pub use i2c::I2cMaster;

pub mod pwm;
pub use pwm::Pwm;

pub mod spi;
pub use spi::SpiMaster;

pub mod serial;
pub use serial::Serial;

pub mod rng;

pub mod sha;
pub use sha::{Sha1, Sha256};

pub mod usbd;
pub use usbd::UsbBus;

pub mod timer;
pub use timer::Timer;

pub mod touch;
pub use touch::TouchSensor;
