#[derive(Copy, Clone, Debug, PartialEq)]
pub enum State {
    NewSession(u8),
    Continue(u8),
}

pub enum Error {
    NewSession,
    NoActivity,
}

pub type Result<T> = core::result::Result<T, Error>;

pub trait ChipDriver {
    fn read(&mut self, buf: &mut [u8]) -> Result<State>;

    fn send(&mut self,buf: &[u8]) -> Result<()>;

    fn frame_size(&self) -> usize;
    //  { 128 }
}
