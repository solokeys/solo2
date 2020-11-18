#![no_std]

pub mod device;

pub use device::{
    FM11NC08,
    Configuration,
    Register,
};

logging::add!(logger);