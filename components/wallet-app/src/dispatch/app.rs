//! App trait for the wallet HID dispatch.
//! Generic over the request message size `N` (default = `REQUEST_SIZE`); the
//! response is the fixed small `ResponseMessage`.

use crate::dispatch::types::{AppResult, Message, ResponseMessage, DEFAULT_MESSAGE_SIZE};

pub trait App<const N: usize = DEFAULT_MESSAGE_SIZE> {
    /// Called when a HID message is received. Write the response into
    /// `response` (pre-cleared) or return an error.
    fn call(&mut self, request: &Message<N>, response: &mut ResponseMessage) -> AppResult;

    /// True while the app has an operation in flight that hasn't yet produced a
    /// response (e.g. a sign waiting on user presence).
    /// While pending the dispatch does not take new requests; it drives `poll`.
    fn is_pending(&self) -> bool {
        false
    }

    /// Advance an in-flight operation without blocking. `Some(result)` = the
    /// operation finished (the response is in `response`); `None` = still
    /// waiting, call again next dispatch tick.
    fn poll(&mut self, _response: &mut ResponseMessage) -> Option<AppResult> {
        None
    }
}
