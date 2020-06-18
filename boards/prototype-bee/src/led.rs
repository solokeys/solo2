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

pub type RedLedPin = pins::Pio1_21;
pub type GreenLedPin = pins::Pio0_5;
pub type BlueLedPin = pins::Pio1_19;

type RedLed = hal::Pin<RedLedPin, pin::state::Special<function::MATCH_OUTPUT2<ctimer::Ctimer3<init_state::Enabled>>>>;
type GreenLed = hal::Pin<GreenLedPin, pin::state::Special<function::MATCH_OUTPUT0<ctimer::Ctimer3<init_state::Enabled>>>>;
type BlueLed = hal::Pin<BlueLedPin, pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer3<init_state::Enabled>>>>;

type PwmDriver = pwm::Pwm<ctimer::Ctimer3<init_state::Enabled>>;

pub struct RgbLed {
    pwm: PwmDriver,
}

impl RgbLed {
    pub fn new(_red: RedLed, _green: GreenLed, _blue: BlueLed, mut pwm: PwmDriver,) -> RgbLed{

        // Xpresso LED (they used same channel for two pins)
        // So blue and red will always get turned on at same time and same intensity.

        pwm.set_duty(RedLed::CHANNEL,0);
        pwm.set_duty(GreenLed::CHANNEL, 0);
        pwm.set_duty(BlueLed::CHANNEL, 0);
        pwm.enable(RedLed::CHANNEL);
        pwm.enable(GreenLed::CHANNEL);
        pwm.enable(BlueLed::CHANNEL);

        Self {
            // red, blue, green,
            pwm,
        }
    }
}

impl rgb_led::RgbLed for RgbLed {
    fn red(&mut self, intensity: u8){
       self.pwm.set_duty(RedLed::CHANNEL, intensity/7 + 1);
    }

    fn green(&mut self, intensity: u8){
        let intensity: u16 = (intensity as u16) * 3/2;
        if intensity > 255 {
            self.pwm.set_duty(GreenLed::CHANNEL, 255);
        } else {
            self.pwm.set_duty(GreenLed::CHANNEL, intensity as u8);
        }
    }

    fn blue(&mut self, intensity: u8) {
        self.pwm.set_duty(BlueLed::CHANNEL, intensity);
    }
}

