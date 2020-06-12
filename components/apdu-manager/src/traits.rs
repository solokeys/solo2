use heapless::ByteBuf;
use iso7816::{Command, response::Result as ResponseResult, Status};


pub enum SourceError {
    NoData,
}

/// Something that ApduManager can read and write unparsed APDUs from/to.
pub trait ApduSource{

    /// Read APDU into given buffer.  Return length of APDU on success.
    fn read_apdu(&mut self, buffer: &mut [u8]) -> nb::Result<u16, SourceError>;

    /// Write response code + APDU
    fn send_apdu(&mut self, code: Status, buffer: &[u8]) -> nb::Result<(), SourceError>;
}

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


// pub type AidBuffer = [u8; 16];

/// Something that can receive and respond APDUs at behest of the ApduManager.
pub trait Applet : Aid {
    /// Given parsed APDU for select command.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn select(&mut self, apdu: Command) -> ResponseResult;

    /// Deselects the applet.  This may be as a result of another applet getting selected.
    /// It would be a good idea for the applet to use this to reset any sensitive state.
    fn deselect(&mut self) -> Result<(), Status>;

    /// Given parsed APDU for applet when selected.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn send_recv(&mut self, apdu: Command) -> ResponseResult;
}