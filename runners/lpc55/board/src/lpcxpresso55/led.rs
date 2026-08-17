use crate::hal::{
    self,
    drivers::pins,
    drivers::pwm,
    peripherals::ctimer,
    typestates::{
        init_state,
        pin::{self, function},
    },
    Iocon,
};

use crate::led::{HwPwmConfig, HwPwmLed};

// OKdo E1 has pins switched for red+blue LED
#[cfg(not(feature = "okdoe1"))]
pub type RedLedPin = pins::Pio1_6;
#[cfg(feature = "okdoe1")]
pub type RedLedPin = pins::Pio1_4;

pub type GreenLedPin = pins::Pio1_7;

#[cfg(not(feature = "okdoe1"))]
pub type BlueLedPin = pins::Pio1_4;
#[cfg(feature = "okdoe1")]
pub type BlueLedPin = pins::Pio1_6;

type RedLed = hal::Pin<
    RedLedPin,
    pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer2<init_state::Enabled>>>,
>;
type GreenLed = hal::Pin<
    GreenLedPin,
    pin::state::Special<function::MATCH_OUTPUT2<ctimer::Ctimer2<init_state::Enabled>>>,
>;
type BlueLed = hal::Pin<
    BlueLedPin,
    pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer2<init_state::Enabled>>>,
>;

type PwmDriver = pwm::Pwm<ctimer::Ctimer2<init_state::Enabled>>;

pub struct Config;

impl HwPwmConfig for Config {
    const CHANNELS: [u8; 3] = [RedLed::CHANNEL, GreenLed::CHANNEL, BlueLed::CHANNEL];
    const MAX_DUTY_SCALE: u32 = 16;

    fn duty(_channel: usize, intensity: u8) -> u16 {
        intensity.into()
    }
}

pub type RgbLed = HwPwmLed<ctimer::Ctimer2<init_state::Enabled>, Config>;

impl RgbLed {
    pub fn new(pwm: PwmDriver, iocon: &mut Iocon<init_state::Enabled>) -> Self {
        let red = RedLedPin::take().unwrap();
        let green = GreenLedPin::take().unwrap();
        let blue = BlueLedPin::take().unwrap();
        Self::with_outputs(pwm, || {
            red.into_match_output(iocon);
            green.into_match_output(iocon);
            blue.into_match_output(iocon);
        })
    }
}
