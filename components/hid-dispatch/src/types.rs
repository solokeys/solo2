
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub enum Error {
    NoResponse,
    InvalidCommand,
}

// 7609 bytes is max message size for ctaphid
type U6144 = <heapless::consts::U4096 as core::ops::Add<heapless::consts::U2048>>::Output;
type U7168 = <U6144 as core::ops::Add<heapless::consts::U1024>>::Output;
pub type U7609 = <U7168 as core::ops::Add<heapless::consts::U441>>::Output;
// pub type U7609 = heapless::consts::U4096;

pub type Message = heapless::ByteBuf<U7609>;
pub type AppResponse = core::result::Result<(), Error>;
pub type InterchangeResponse = core::result::Result<Message, Error>;

pub use crate::command::Command;

interchange::interchange! {
    HidInterchange: ((Command, crate::types::Message), crate::types::InterchangeResponse)
}

