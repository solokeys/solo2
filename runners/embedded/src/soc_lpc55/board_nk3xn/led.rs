use lpc55_hal::{
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
use crate::soc::traits::rgb_led;

pub enum Color {
    Red,
    Green,
    Blue,
}

pub type RedLedPin = pins::Pio0_5;
pub type GreenLedPin = pins::Pio1_21;
pub type BlueLedPin = pins::Pio1_19;

type RedLed = lpc55_hal::Pin<
    RedLedPin,
    pin::state::Special<function::MATCH_OUTPUT0<ctimer::Ctimer3<init_state::Enabled>>>
>;
type GreenLed = lpc55_hal::Pin<
    GreenLedPin,
    pin::state::Special<function::MATCH_OUTPUT2<ctimer::Ctimer3<init_state::Enabled>>>
>;
type BlueLed = lpc55_hal::Pin<
    BlueLedPin,
    pin::state::Special<function::MATCH_OUTPUT1<ctimer::Ctimer3<init_state::Enabled>>>
>;

type PwmDriver = pwm::Pwm<ctimer::Ctimer3<init_state::Enabled>>;

pub struct RgbLed {
    pwm: PwmDriver,
}

impl RgbLed {
    pub fn new(
        mut pwm: PwmDriver,
        iocon: &mut Iocon<init_state::Enabled>
    ) -> RgbLed{

        let red = RedLedPin::take().unwrap();
        let green = GreenLedPin::take().unwrap();
        let blue = BlueLedPin::take().unwrap();

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

        pwm.scale_max_duty_by(8);

        Self {
            pwm,
        }
    }
}

impl rgb_led::RgbLed for RgbLed {
    fn red(&mut self, intensity: u8){
        self.pwm.set_duty(RedLed::CHANNEL, (intensity/2) as u16);
    }

    fn green(&mut self, intensity: u8){
        self.pwm.set_duty(GreenLed::CHANNEL, (intensity as u16) * 3);
    }

    fn blue(&mut self, intensity: u8) {
        self.pwm.set_duty(BlueLed::CHANNEL, (intensity as u16) * 8);
    }
}

