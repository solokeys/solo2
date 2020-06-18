use iso7816::{Command, response::Data, Status};

pub enum AppletResponse {
    Respond(Data),
    Defer,
}

impl Default for AppletResponse {
    fn default() -> Self {
        AppletResponse::Respond(Default::default())
    }
}

pub type Result = core::result::Result<AppletResponse, Status>;

pub type ScratchBuffer = [u8; 4096];

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



/// An App can receive and respond APDUs at behest of the ApduManager.
pub trait Applet : Aid {
    /// Given parsed APDU for select command.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    /// Alternatively, the app can defer the response until later by returning it in `poll()`.
    fn select(&mut self, apdu: Command) -> Result;

    /// Deselects the applet.  This may be as a result of another applet getting selected.
    /// It would be a good idea for the applet to use this to reset any sensitive state.
    fn deselect(&mut self) -> core::result::Result<(), Status>;

    /// Given parsed APDU for applet when selected.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn send_recv(&mut self, apdu: Command) -> Result;

    /// Called repeatedly for the selected applet.
    /// Applet could choose to defer a response in `send_recv`, and send a reply later here.
    fn poll(&mut self, _buffer: &mut ScratchBuffer) -> Result {
        Ok(AppletResponse::Defer)
    }
}