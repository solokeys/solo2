pub mod button;
pub mod led;
pub mod nfc;

use crate::hal::traits::wg::timer::CountDown;
use crate::hal::{
    self, drivers::clocks::Clocks, drivers::Pwm, drivers::Timer, peripherals::ctimer,
    typestates::init_state::Unknown, Enabled,
};

pub use button::ThreeButtons;
pub use led::RgbLed;

/// Builds the LED and buttons. Timer allocation is board-specific, so the caller
/// passes every candidate peripheral and the board takes what it needs.
#[allow(clippy::too_many_arguments)]
pub fn new_ui(
    active: bool,
    _adc: &mut Option<hal::Adc<Enabled>>,
    ctimer1: ctimer::Ctimer1,
    ctimer2: ctimer::Ctimer2,
    _ctimer3: ctimer::Ctimer3,
    _dma: hal::Dma<Unknown>,
    syscon: &mut hal::Syscon,
    clocks: Clocks,
    gpio: &mut hal::Gpio<Enabled>,
    iocon: &mut hal::Iocon<Enabled>,
) -> (Option<RgbLed>, Option<ThreeButtons>) {
    if !active {
        return (None, None);
    }

    let rgb = RgbLed::new(
        Pwm::new(ctimer2.enabled(syscon, clocks.support_1mhz_fro_token().unwrap())),
        iocon,
    );
    let buttons = ThreeButtons::new(
        Timer::new(ctimer1.enabled(syscon, clocks.support_1mhz_fro_token().unwrap())),
        gpio,
        iocon,
    );

    (Some(rgb), Some(buttons))
}

/// Holds the boot-to-bootrom blink for the duration of `timer`.
pub fn hold_blink<T: hal::peripherals::ctimer::Ctimer<Enabled>>(timer: &mut Timer<T>) {
    nb::block!(timer.wait()).ok();
}
