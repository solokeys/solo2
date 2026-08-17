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

pub type RedLedPin = pins::Pio0_5;
pub type GreenLedPin = pins::Pio1_21;
pub type BlueLedPin = pins::Pio1_19;

type RedLed = hal::Pin<
    RedLedPin,
    pin::state::Special<function::MATCH_OUTPUT0<ctimer::Ctimer3<init_state::Enabled>>>,
>;
type GreenLed = hal::Pin<
    GreenLedPin,
    pin::state::Special<function::MATCH_OUTPUT2<ctimer::Ctimer3<init_state::Enabled>>>,
>;
type BlueLed = hal::Pin<
    BlueLedPin,
    pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer3<init_state::Enabled>>>,
>;

type PwmDriver = pwm::Pwm<ctimer::Ctimer3<init_state::Enabled>>;

pub struct Config;

impl HwPwmConfig for Config {
    const CHANNELS: [u8; 3] = [RedLed::CHANNEL, GreenLed::CHANNEL, BlueLed::CHANNEL];
    const MAX_DUTY_SCALE: u32 = 8;

    fn duty(channel: usize, intensity: u8) -> u16 {
        match channel {
            0 => (intensity / 2) as u16,
            1 => (intensity as u16) * 3,
            _ => (intensity as u16) * 8,
        }
    }
}

pub type RgbLed = HwPwmLed<ctimer::Ctimer3<init_state::Enabled>, Config>;

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
