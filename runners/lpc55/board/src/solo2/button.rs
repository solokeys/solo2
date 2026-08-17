use crate::hal::{
    self,
    drivers::pins,
    drivers::touch::{ButtonPins, TouchSensorChannel},
    peripherals::ctimer,
    typestates::{init_state, ClocksSupportTouchToken},
};

use crate::button::{ButtonConfig, TouchButtons};
use crate::traits::buttons::Button;

pub type ButtonAPin = pins::Pio0_23;
pub type ButtonBPin = pins::Pio0_31;
pub type ButtonMiddlePin = pins::Pio0_15;

type Adc = hal::peripherals::adc::Adc<init_state::Enabled>;
type Dma = hal::peripherals::dma::Dma<init_state::Enabled>;
type AdcTimer = ctimer::Ctimer1<init_state::Enabled>;
type SampleTimer = ctimer::Ctimer2<init_state::Enabled>;

pub struct Config;

impl ButtonConfig for Config {
    type ButtonA = ButtonAPin;
    type ButtonB = ButtonBPin;
    type ButtonMiddle = ButtonMiddlePin;
    const THRESHOLDS: [u32; 3] = [12_000; 3];
    const CONFIDENCE: u32 = 5;
    const SCAN: &'static [Button] = &[Button::A, Button::B, Button::Middle];

    fn channel(button: Button) -> TouchSensorChannel {
        match button {
            Button::A => TouchSensorChannel::Channel1,
            Button::B => TouchSensorChannel::Channel2,
            Button::Middle => TouchSensorChannel::Channel3,
        }
    }
}

pub type ThreeButtons = TouchButtons<Config>;

impl ThreeButtons {
    pub fn new(
        adc: Adc,
        adc_timer: AdcTimer,
        sample_timer: SampleTimer,
        dma: &mut Dma,
        token: ClocksSupportTouchToken,
        gpio: &mut hal::Gpio<hal::Enabled>,
        iocon: &mut hal::Iocon<hal::Enabled>,
    ) -> Self {
        let a = ButtonAPin::take().unwrap().into_analog_input(iocon, gpio);
        let b = ButtonBPin::take().unwrap().into_analog_input(iocon, gpio);
        let middle = ButtonMiddlePin::take()
            .unwrap()
            .into_analog_input(iocon, gpio);
        Self::from_pins(
            adc,
            adc_timer,
            sample_timer,
            dma,
            token,
            iocon,
            ButtonPins(a, b, middle),
        )
    }
}
