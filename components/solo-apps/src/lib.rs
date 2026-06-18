#![no_std]

//! Shared CTAP/NFC/apps building blocks for the lpc55 and nrf52840dk runners.
//!
//! Provides the trussed dispatch, the canonical FIDO config, the client/backend
//! plumbing, and the generic NDEF-suppression wrappers. Board-specific assembly
//! (storage, HAL, NDEF timebase, `UserInterface`, `Apps::new`) stays in each
//! runner.

pub mod client;
pub mod config;
pub mod dispatch;
pub mod ndef;
