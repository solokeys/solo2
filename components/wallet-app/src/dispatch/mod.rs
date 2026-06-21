//! Dispatch module for Wallet HID protocol.

pub mod app;
#[allow(clippy::module_inception)]
pub mod dispatch;
pub mod types;

pub use app::App;
pub use dispatch::Dispatch;
pub use types::{
    Channel, Error, InterchangeResponse, Message, Requester, Responder, ResponseMessage,
    DEFAULT_MESSAGE_SIZE, REQUEST_SIZE, RESPONSE_SIZE,
};
