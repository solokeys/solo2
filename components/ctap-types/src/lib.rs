#![cfg_attr(not(test), no_std)]
// #![no_std]

//! `ctap-types` maps the various types involved in the FIDO CTAP protocol
//! to Rust structures consisting of `heapless` data types.
//!
//! We currently follow the non-public editor's draft dated 19 March 2019.
//! It still uses `FIDO_2_1_PRE` to signal new commands, but uses non-vendor
//! API numbering (e.g. 0xA for credential management).
//!
//! It also contains a lightweight CBOR deserializer, as the existing `serde_cbor`
//! creates very large code.
//!
//! The various transport protocols (USB, NFC, BLE) are expected to handle
//! low-level protocol details and deserialize requests / serialize responses,
//! so the authenticator logic is decoupled from these details.

#[macro_use]
extern crate ufmt_macros;

#[macro_use]
extern crate delog;
generate_macros!();

pub use heapless::{consts, ArrayLength, String, Vec};
pub use heapless::spsc::{Consumer, Producer, Queue};
pub use heapless::ByteBuf;
pub type ByteBuf16 = ByteBuf<consts::U16>;
pub type ByteBuf32 = ByteBuf<consts::U32>;

pub mod authenticator;
pub mod cose;
pub mod ctap1;
pub mod ctap2;
pub mod operation;
pub mod rpc;
pub mod serde;
pub mod sizes;
pub mod webauthn;

#[cfg(test)]
mod tests {
}
