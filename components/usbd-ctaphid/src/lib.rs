#![no_std]

/*!
usbd-ctaphid

See "proposed standard":
https://fidoalliance.org/specs/fido-v2.0-ps-20190130/fido-client-to-authenticator-protocol-v2.0-ps-20190130.html#usb

*/

// use heapless_bytes as bytes;

// pub mod authenticator;

pub mod constants;
pub mod class;
pub use class::CtapHid;
pub mod pipe;

// #[cfg(feature = "insecure-ram-authenticator")]
// pub mod insecure;

// #[cfg(not(feature = "logging"))]
// mod logging;

// // TODO: not really sure what's going on here...
// // Goal: have `logging` feature, that can be completely turned off

// #[cfg(feature = "logging")]
// // use ufmt::UnstableDoAsFormatter;

// #[cfg(feature = "logging")]
// use funnel::debug;

// #[cfg(feature = "logging")]
// use funnel::error;

// pub mod types;
