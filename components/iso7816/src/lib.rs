#![cfg_attr(not(test), no_std)]
// #![no_std]

#[allow(non_camel_case_types)]
pub type U3072 = <heapless::consts::U2048 as core::ops::Add<heapless::consts::U1024>>::Output;
#[allow(non_camel_case_types)]
pub type MAX_COMMAND_DATA = U3072;

// 7816-4, 8.2.1.2
pub type Aid = heapless::ByteBuf<heapless::consts::U16>;

pub mod command;
pub mod response;

pub use command::Command;
pub use command::instruction::Instruction;
pub use response::Response;
pub use response::status::Status;

// NB: This library is not necessarily the optimal location
// to define these two interchanges. However, we want to avoid
// making `apdu-dispatch` a dependency of the `usbd-ccid` and
// `nfc-device` libraries, and here seems lightweight enough for now.

#[cfg(feature = "contact-interchange")]
interchange::interchange! {
    ContactInterchange: (command::Data, response::Data)
}

#[cfg(feature = "contactless-interchange")]
interchange::interchange! {
    ContactlessInterchange: (command::Data, response::Data)
}
