#![cfg_attr(not(test), no_std)]
#![deny(warnings)]
#![allow(unexpected_cfgs)] // Allow log-trace from delog macro

#[macro_use]
extern crate delog;
generate_macros!();

#[macro_use(hex)]
extern crate hex_literal;

pub mod authenticator;
pub use authenticator::Authenticator;
pub mod chain;
pub use chain::Chain;
pub mod command;
pub use command::Command;
pub mod consent;
pub mod derivation_path;
pub mod eth;
pub use derivation_path::DerivationPath;
pub mod key_derivation;
pub mod signing;
pub mod state;
pub use state::{SecretState, SecretType, State};

#[cfg(feature = "dispatch")]
pub mod dispatch;

#[cfg(all(feature = "usbd", feature = "dispatch"))]
pub mod usbd;

pub mod nfc;

// Wallet App ID
pub const WALLET_AID: &[u8] = &hex!("E0 01 01 01 01");

// APDU constants
pub const APDU_CLA: u8 = 0xE0;
pub const APDU_SUCCESS: u16 = 0x9000;

// USB enumeration identity (vid/pid + strings) is owned by the runner. The
// default is the SoloKeys Solo 2; a user-supplied vid/pid can override it.
