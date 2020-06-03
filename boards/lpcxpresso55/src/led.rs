use crate::hal::{
    self,
    drivers::pins,
    drivers::pwm,
    peripherals::ctimer,
    typestates::{
        init_state,
        pin::{
            self,
            function,
        },
    },
    traits::wg::Pwm,
};
pub enum Color {
    Red,
    Green,
    Blue,
}

use solo_bee_traits::rgb_led;

pub type RedLedPin = pins::Pio1_4;
pub type GreenLedPin = pins::Pio1_7;
pub type BlueLedPin = pins::Pio1_6;

type RedLed = hal::Pin<RedLedPin, pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer2<init_state::Enabled>>>>;
type GreenLed = hal::Pin<GreenLedPin, pin::state::Special<function::MATCH_OUTPUT2<ctimer::Ctimer2<init_state::Enabled>>>>;
type BlueLed = hal::Pin<BlueLedPin, pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer2<init_state::Enabled>>>>;

type PwmDriver = pwm::Pwm<ctimer::Ctimer2<init_state::Enabled>>;

pub struct RgbLed {
    // red: RedLed,
    // green: GreenLed,
    // blue: BlueLed,
    pwm: PwmDriver,
}

impl RgbLed {
    pub fn new(_red: RedLed, _green: GreenLed, _blue: BlueLed, mut pwm: PwmDriver,) -> RgbLed {

        // Xpresso LED (they used same channel for two pins)
        // So blue and red will always get turned on at same time and same intensity.

        pwm.set_duty(RedLed::CHANNEL,0);
        pwm.set_duty(GreenLed::CHANNEL, 0);
        pwm.set_duty(BlueLed::CHANNEL, 0);
        pwm.enable(RedLed::CHANNEL);
        pwm.enable(GreenLed::CHANNEL);
        pwm.enable(BlueLed::CHANNEL);

        RgbLed {
            // red, blue, green,
            pwm,
        }
    }
}

impl rgb_led::RgbLed for RgbLed {
    fn red(&mut self, intensity: u8){
        self.pwm.set_duty(RedLed::CHANNEL, intensity);
    }

    fn green(&mut self, intensity: u8){
        self.pwm.set_duty(GreenLed::CHANNEL, intensity);
    }

    fn blue(&mut self, intensity: u8) {
        self.pwm.set_duty(BlueLed::CHANNEL, intensity);
    }
}
