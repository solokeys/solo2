use iso7816::{Command, response::Data, Status};

pub type Result = core::result::Result<Response, Status>;

pub enum Response {
    Respond(Data),
    Defer,
}

impl Default for Response {
    fn default() -> Self {
        Response::Respond(Default::default())
    }
}

/// The Aid is used to determine whether or not the App will be selected.
/// Only `aid()` and `right_truncated_length()` need to be implemented.
pub trait Aid {

    fn aid(&self) -> &'static [u8];

    fn right_truncated_length(&self) -> usize;

    fn len(&self) -> usize {
        self.aid().len()
    }

    fn full(&self) -> &'static [u8] {
        self.aid()
    }

    fn right_truncated(&self) -> &'static [u8] {
        &self.aid()[..self.right_truncated_length()]
    }

    fn pix(&self) -> &'static [u8] {
        &self.aid()[5..]
    }

    fn rid(&self) -> &'static [u8] {
        &self.aid()[..5]
    }
}



/// An App can receive and respond APDUs at behest of the ApduDispatch.
pub trait Applet : Aid {
    /// Given parsed APDU for select command.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    /// Alternatively, the app can defer the response until later by returning it in `poll()`.
    fn select(&mut self, apdu: &Command) -> Result;

    /// Deselects the applet. This is the result of another applet getting selected.
    /// Applet should clear any sensitive state and reset security indicators.
    fn deselect(&mut self);

    /// Given parsed APDU for applet when selected.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn call(&mut self, apdu: &Command) -> Result;

    /// Called repeatedly for the selected applet.
    /// Applet could choose to defer a response in `send_recv`, and send a reply later here.
    fn poll(&mut self) -> Result {
        Ok(Response::Defer)
    }
}
