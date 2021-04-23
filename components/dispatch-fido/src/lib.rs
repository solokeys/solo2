//! # dispatch-fido
//!
//! This library implements the `apdu-dispatch` and `ctaphid-dispatch` App traits
//! for the `fido-authenticator`, allowing it to be called over both interfaces
//! in the Solo 2 security key.
#![no_std]

#[macro_use]
extern crate delog;
generate_macros!();

pub mod fido;
pub use fido::*;

pub mod cbor;
