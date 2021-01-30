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
    Iocon,
};
pub enum Color {
    Red,
    Green,
    Blue,
}

use crate::traits::rgb_led;

pub type RedLedPin = pins::Pio1_4;
pub type GreenLedPin = pins::Pio1_7;
pub type BlueLedPin = pins::Pio1_6;

type RedLed = hal::Pin<RedLedPin, pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer2<init_state::Enabled>>>>;
type GreenLed = hal::Pin<GreenLedPin, pin::state::Special<function::MATCH_OUTPUT2<ctimer::Ctimer2<init_state::Enabled>>>>;
type BlueLed = hal::Pin<BlueLedPin, pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer2<init_state::Enabled>>>>;

type PwmDriver = pwm::Pwm<ctimer::Ctimer3<init_state::Enabled>>;

pub struct RgbLed {
    // red: RedLed,
    // green: GreenLed,
    // blue: BlueLed,
    pwm: PwmDriver,
}

impl RgbLed {
    pub fn new(
        mut pwm: PwmDriver,
        iocon: &mut Iocon<init_state::Enabled>
    ) -> RgbLed{

        let red = RedLedPin::take().unwrap();
        let green = RedLedPin::take().unwrap();
        let blue = RedLedPin::take().unwrap();

        pwm.set_duty(RedLed::CHANNEL,0);
        pwm.set_duty(GreenLed::CHANNEL, 0);
        pwm.set_duty(BlueLed::CHANNEL, 0);
        pwm.enable(RedLed::CHANNEL);
        pwm.enable(GreenLed::CHANNEL);
        pwm.enable(BlueLed::CHANNEL);

        // Don't set to output until after duty cycle is set to zero to save power.
        red.into_match_output(iocon);
        green.into_match_output(iocon);
        blue.into_match_output(iocon);

        pwm.scale_max_duty_by(16);

        Self {
            pwm,
        }
    }
}

impl rgb_led::RgbLed for RgbLed {
    fn red(&mut self, intensity: u8){
        self.pwm.set_duty(RedLed::CHANNEL, intensity.into());
    }

    fn green(&mut self, intensity: u8){
        self.pwm.set_duty(GreenLed::CHANNEL, intensity.into());
    }

    fn blue(&mut self, intensity: u8) {
        self.pwm.set_duty(BlueLed::CHANNEL, intensity.into());
    }
}
