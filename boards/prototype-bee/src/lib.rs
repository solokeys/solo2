#![no_std]

pub use cortex_m_rt as rt;
pub use lpc55_hal as hal;

pub mod button;
pub mod led;

pub type Timer = lpc55_common::Timer<hal::peripherals::ctimer::Ctimer0<hal::typestates::init_state::Enabled>>;