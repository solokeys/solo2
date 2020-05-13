// #![cfg_attr(not(test), no_std)]
#![no_std]

// prevent a spurious error message: https://github.com/rust-lang/rust/issues/54010
// UNFORTUNATELY: with #![cfg(test)], no longer compiles for no_std,
// with #[cfg(test)] error still shown
// #[cfg(test)]
// extern crate std;

#[cfg(not(feature = "debug-logs"))]
#[macro_use(info)]
extern crate funnel;

#[cfg(feature = "debug-logs")]
#[macro_use(debug,info)]
extern crate funnel;

#[cfg(not(feature = "debug-logs"))]
#[macro_use]
macro_rules! debug { ($($tt:tt)*) => {{ core::result::Result::<(), core::convert::Infallible>::Ok(()) }} }

pub mod api;
pub mod client;
pub mod config;
pub mod error;
pub mod mechanisms;
pub mod pipe;
pub mod service;
pub mod store;
pub mod types;

pub use api::Reply;
pub use error::Error;
pub use client::Client;
pub use service::Service;

pub use ctap_types::{ArrayLength, Bytes, consts, serde::{cbor_serialize, cbor_serialize_bytes, cbor_deserialize}};
pub fn cbor_serialize_bytebuf<N: heapless_bytes::ArrayLength<u8>, T: serde::Serialize>(object: &T) -> core::result::Result<Bytes<N>, ctap_types::serde::Error> {
    let mut data = heapless_bytes::Bytes::<N>::new();
    ctap_types::serde::cbor_serialize_bytes(object, &mut data)?;
    Ok(data)
}

// #[cfg(test)]
// mod tests;
