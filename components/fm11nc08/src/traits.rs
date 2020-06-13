
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum NfcState {
    Idle,
    Recieving,
    Transmitting,
}

pub enum NfcError{
    NoActivity,
}

pub trait NfcDevice {
    fn get_state(&mut self, ) -> NfcState;

    fn read(&mut self, buf: &mut [u8]) -> nb::Result<u8, NfcError>;

    fn send(&mut self,buf: &[u8]) -> nb::Result<(), NfcError>;


    fn write_but_dont_send(&mut self,buf: &[u8]);
}