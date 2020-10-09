#![no_std]

pub mod device;

pub mod traits;

// #[derive(Copy, Clone, Debug, PartialEq)]
// pub enum State {
//     NewSession(u8),
//     Continue(u8),
// }

// pub enum Error {
//     NewSession,
//     NoActivity,
// }

// pub type Result<T> = core::result::Result<T, Error>;

// pub trait Device {
//     fn read(&mut self, buf: &mut [u8]) -> Result<State>;

//     fn send(&mut self,buf: &[u8]) -> Result<()>;

//     // typical value: 128
//     fn frame_size(&self) -> usize;
// }


pub use device::{
    FM11NC08,
    Configuration,
    Register,
};

logging::add!(logger);
