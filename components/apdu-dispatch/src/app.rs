pub use iso7816::{Command, Data, Status};
pub type Result = iso7816::Result<()>;

pub use crate::{ArrayLength, dispatch::Interface};

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
pub trait App<C: ArrayLength<u8>, R: ArrayLength<u8>>: Aid {
    /// Given parsed APDU for select command.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    /// Alternatively, the app can defer the response until later by returning it in `poll()`.
    fn select(&mut self, apdu: &Command<C>, reply: &mut Data<R>) -> Result;

    /// Deselects the app. This is the result of another app getting selected.
    /// App should clear any sensitive state and reset security indicators.
    fn deselect(&mut self);

    /// Given parsed APDU for app when selected.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn call(&mut self, interface: Interface, apdu: &Command<C>, reply: &mut Data<R>) -> Result;

}
