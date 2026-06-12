//! Error and Result types.
//!
//! Currently we just use `anyhow`. In time, library errors should be modeled properly,
//! and implemented e.g. using `thiserror`.
//!
pub type Error = anyhow::Error;
pub type Result<T> = anyhow::Result<T>;
