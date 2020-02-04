#![cfg_attr(not(test), no_std)]

pub mod api;
pub mod client;
pub mod config;
pub mod error;
pub mod pipe;
pub mod service;
pub mod types;

pub use api::{Request, Reply};
pub use error::{Error, FutureResult};
pub use client::RawClient;
pub use service::Service;

#[cfg(test)]
mod tests;
