pub mod button;
pub mod led;

use crate::hal;

pub type Timer = crate::traits::Timer<hal::peripherals::ctimer::Ctimer0<hal::typestates::init_state::Enabled>>;
