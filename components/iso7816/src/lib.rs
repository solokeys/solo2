#![cfg_attr(not(test), no_std)]
// #![no_std]

pub use heapless_bytes::Bytes as Bytes;

#[allow(non_camel_case_types)]
pub type U3072 = <heapless::consts::U2048 as core::ops::Add<heapless::consts::U1024>>::Output;
#[allow(non_camel_case_types)]
pub type MAX_COMMAND_DATA = U3072;

// 7816-4, 8.2.1.2
pub type Aid = Bytes<heapless::consts::U16>;

pub mod command;
pub mod response;

pub use command::Command;
pub use command::instruction::Instruction;
pub use response::Response;
pub use response::status::Status;
