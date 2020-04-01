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
pub mod types;

pub use api::Reply;
pub use error::Error;
pub use client::Client;
pub use service::Service;

pub use ctap_types::serde::{cbor_serialize, cbor_serialize_bytes, cbor_deserialize};

// #[cfg(test)]
// mod tests;
