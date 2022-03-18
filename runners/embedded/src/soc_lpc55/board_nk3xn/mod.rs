pub mod button;
pub mod led;

pub const BOARD_NAME: &'static str = "nk3xn";

pub type PwmTimer = lpc55_hal::peripherals::ctimer::Ctimer3<lpc55_hal::typestates::init_state::Unknown>;
