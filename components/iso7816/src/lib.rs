#![cfg_attr(not(test), no_std)]
// #![no_std]

pub use heapless_bytes::Bytes as Bytes;

// 7816-4, 8.2.1.2
pub type Aid = Bytes<heapless::consts::U16>;

#[derive(Copy, Clone, PartialEq)]
pub enum Interface {
    Contact,
    Contactless,
}

pub type Result<T> = core::result::Result<T, Status>;

pub mod command;
pub mod response;

pub use command::Command;
pub use command::instruction::Instruction;
pub use response::{Data, Response};
pub use response::status::Status;
