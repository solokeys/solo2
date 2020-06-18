
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum NfcState {
    NewSession(u8),
    Continue(u8),
}

pub enum NfcError {
    NewSession,
    NoActivity,
}

pub trait NfcDevice {
    fn read(&mut self, buf: &mut [u8]) -> Result<NfcState, NfcError>;

    fn send(&mut self,buf: &[u8]) -> Result<(), NfcError>;

    // fn wait(&mut self) -> nb::Result<(), NfcError>;

    fn frame_size(&self) -> u8 { 127 }

    // fn write_but_dont_send(&mut self,buf: &[u8]);
}