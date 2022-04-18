#![no_std]

/*!
usbd-ctaphid

See "proposed standard":
https://fidoalliance.org/specs/fido-v2.0-ps-20190130/fido-client-to-authenticator-protocol-v2.0-ps-20190130.html#usb

*/

#[macro_use]
extern crate delog;
generate_macros!();

// use heapless_bytes as bytes;

// pub mod authenticator;

pub mod constants;
pub mod class;
pub use class::CtapHid;
pub mod pipe;
pub mod types;

/// major/minor/build version bytes returned in CTAPHID_INIT
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub build: u8,
}

