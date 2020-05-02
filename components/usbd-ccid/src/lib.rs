#![no_std]

//! https://www.usb.org/sites/default/files/DWG_Smart-Card_CCID_Rev110.pdf
//! https://www.usb.org/sites/default/files/DWG_Smart-Card_USB-ICC_ICCD_rev10.pdf

pub mod constants;
pub mod class;
pub mod pipe;

pub use class::Ccid;