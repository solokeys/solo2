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

use board_traits::rgb_led;

pub type RedLedPin = pins::Pio1_4;
pub type GreenLedPin = pins::Pio1_7;
pub type BlueLedPin = pins::Pio1_6;

type RedLed = hal::Pin<RedLedPin, pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer2<init_state::Enabled>>>>;
type GreenLed = hal::Pin<GreenLedPin, pin::state::Special<function::MATCH_OUTPUT2<ctimer::Ctimer2<init_state::Enabled>>>>;
type BlueLed = hal::Pin<BlueLedPin, pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer2<init_state::Enabled>>>>;

type RedLedUnenabled = hal::Pin<RedLedPin, pin::state::Unused>;
type GreenLedUnenabled = hal::Pin<GreenLedPin,pin::state::Unused>;
type BlueLedUnenabled = hal::Pin<BlueLedPin, pin::state::Unused>;

type PwmDriver = pwm::Pwm<ctimer::Ctimer2<init_state::Enabled>>;

pub struct RgbLed {
    // red: RedLed,
    // green: GreenLed,
    // blue: BlueLed,
    pwm: PwmDriver,
}

impl RgbLed {
    pub fn new(
        red: RedLedUnenabled,
        green: GreenLedUnenabled,
        blue: BlueLedUnenabled,
        mut pwm: PwmDriver,
        iocon: &mut Iocon<init_state::Enabled>
    ) -> RgbLed{

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
