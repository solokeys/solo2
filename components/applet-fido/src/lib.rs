#![no_std]

pub mod fido;
pub use fido::*;

pub mod cbor;

logging::add!(logger);