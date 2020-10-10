//! `nfc-device` chip driver implementation for the FM11NC08 NFC Channel Chip
//!
//! This chip allows adding NFC functionality, including energy harvesting,
//! to an existing microcontroller design. This implementation only covers
//! the ISO 14443-A, Level 4, SPI variant.
//!
//! [fm-chip-url]: http://www.fm-chips.com/nfc-channel-ics.html
//! [fmsh-url]: http://eng.fmsh.com/2ac114a8-ce2d-aa9d-cef4-ad3919eeb513/

#![no_std]

pub mod chip;

pub use chip::Fm11Nc08S as Chip;
pub use chip::types::Configuration;

logging::add!(logger);
