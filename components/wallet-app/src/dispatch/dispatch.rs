//! Dispatch layer for Wallet HID protocol — interchange 0.3.

use crate::dispatch::app::App;
use crate::dispatch::types::{AppResult, Error, Responder, ResponseMessage, DEFAULT_MESSAGE_SIZE};

pub struct Dispatch<'pipe, const N: usize = DEFAULT_MESSAGE_SIZE> {
    responder: Responder<'pipe, N>,
}

impl<'pipe, const N: usize> Dispatch<'pipe, N> {
    pub fn new(responder: Responder<'pipe, N>) -> Self {
        Self { responder }
    }

    #[inline(never)]
    fn reply_with_error(&mut self, error: Error) {
        self.responder.respond(Err(error).into()).ok();
    }

    /// Send a finished `AppResult` + its response buffer back to the host.
    #[inline(never)]
    fn respond_result(&mut self, result: AppResult, response_buffer: ResponseMessage) {
        if let Err(error) = result {
            self.reply_with_error(error);
        } else {
            self.responder.respond(Ok(response_buffer).into()).ok();
        }
    }

    /// Poll the dispatch. Drives a pending operation (a sign waiting on user
    /// presence) without blocking, or picks up a fresh request. Returns whether
    /// any work was done.
    pub fn poll(&mut self, app: &mut dyn App<N>) -> bool {
        if app.is_pending() {
            // An operation is in flight — advance it without blocking. Only
            // respond once it finishes; while still waiting, send nothing so
            // idle keeps cycling (and the runner keeps servicing NFC).
            let mut response_buffer = ResponseMessage::new();
            if let Some(result) = app.poll(&mut response_buffer) {
                self.respond_result(result, response_buffer);
            }
            true
        } else if let Some(request) = self.responder.take_request() {
            let mut response_buffer = ResponseMessage::new();
            let result = app.call(&request, &mut response_buffer);
            // If the call started a pending operation (e.g. a sign just kicked
            // off the consent wait), do NOT respond yet — `poll` will.
            if !app.is_pending() {
                self.respond_result(result, response_buffer);
            }
            true
        } else {
            false
        }
    }
}
