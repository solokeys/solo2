
pub mod nfc {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub enum State {
        NewSession(u8),
        Continue(u8),
    }

    pub enum Error {
        NewSession,
        NoActivity,
    }

    pub trait Device {
        fn read(&mut self, buf: &mut [u8]) -> Result<State, Error>;

        fn send(&mut self,buf: &[u8]) -> Result<(), Error>;

        // fn wait(&mut self) -> nb::Result<(), NfcError>;

        fn frame_size(&self) -> u8 { 127 }

        // fn write_but_dont_send(&mut self,buf: &[u8]);
    }
}