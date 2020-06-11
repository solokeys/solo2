
use crate::Apdu;

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Error {
    Success = 0x9000,
    SwWrongLength = 0x6700,
    SwCondUseNotSatisfied = 0x6985,
    SwFileNotFound = 0x6A82,
    SwUnknown = 0x6F00,
    SwInsNotSupported = 0x6D00,
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Ins {
    Select = 0xA4,
    ReadBinary = 0xB0,
}

pub enum SourceError {
    NoData,
}

/// Something that ApduManager can read and write APDUs from/to.
pub trait ApduSource{

    /// Read APDU into given buffer.  Return length of APDU on success.
    fn read_apdu(&mut self, buffer: &mut [u8]) -> nb::Result<u16, SourceError>;

    /// Write response code + APDU
    fn send_apdu(&mut self, code: Error, buffer: &[u8]) -> nb::Result<(), SourceError>;
}

pub type AidBuffer = [u8; 16];

/// Something that can receive and respond APDUs at behest of the ApduManager.
pub trait Applet{

    /// AID should be 0 padded if needed.
    // const AID: AidBuffer;

    fn aid(&self) -> &AidBuffer;

    /// Given parsed APDU for select command. 
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn select(&mut self, apdu: &mut Apdu) -> Result<u16, crate::traits::Error>;

    /// Deselects the applet.  This may be as a result of another applet getting selected.
    /// It would be a good idea for the applet to use this to reset any sensitive state.
    fn deselect(&mut self) -> Result<(), crate::traits::Error>;

    /// Given parsed APDU for applet when selected.
    /// Write response data back to buf, and return length of payload.  Return APDU Error code on error.
    fn send_recv(&mut self, apdu: &mut Apdu) -> Result<u16, crate::traits::Error>;
}