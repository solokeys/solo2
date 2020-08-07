#![no_std]

pub mod fido;
pub use fido::*;

logging::add!(logger);