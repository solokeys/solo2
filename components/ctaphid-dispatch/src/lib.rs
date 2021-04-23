//! # ctaphid-dispatch
//!
//! This library defines a concept of CTAPHID apps, which declare
//! CTAPHID commands, which are then dispatched to them.
//!
//! The intention is for non-FIDO authenticator apps to be able
//! to extend the CTAPHID interface with additional functionality.
//!
//! For instance, the Solo 2 management app.
#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate delog;
generate_macros!();

pub mod app;
pub mod types;
pub mod command;
pub mod dispatch;
