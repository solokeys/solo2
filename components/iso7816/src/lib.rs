#![cfg_attr(not(test), no_std)]
// #![no_std]

#[allow(non_camel_case_types)]
pub type U3076 = <heapless::consts::U2048 as core::ops::Add<heapless::consts::U1024>>::Output;
#[allow(non_camel_case_types)]
pub type MAX_COMMAND_DATA = U3076;

pub mod command;
pub mod response;

pub use command::Command;
pub use command::instruction::Instruction;
pub use response::Response;
pub use response::status::Status;
