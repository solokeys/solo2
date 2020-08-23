use core::convert::Infallible;
use crate::hal::{
    self,
    drivers::pins,
    drivers::Pin,
    drivers::touch::{Compare, Edge, TouchSensor, ButtonPins, TouchSensorChannel},
};
use crate::hal::peripherals::{
    ctimer,
};
use trussed_board::buttons::{
    self,
    Button,
};
use crate::hal::typestates::{
    init_state,
    pin::PinId,
    pin::state::{Special, Analog},
    pin::gpio::direction,
    pin::function,
    ClocksSupportTouchToken
};


pub type ChargeMatchPin = pins::Pio1_16;
pub type ButtonTopPin = pins::Pio0_23;
pub type ButtonBotPin = pins::Pio0_31;
pub type ButtonMidPin = pins::Pio0_15;

type Adc = hal::peripherals::adc::Adc<init_state::Enabled>;
type Dma = hal::peripherals::dma::Dma<init_state::Enabled>;

type AdcTimer = ctimer::Ctimer1<init_state::Enabled>;
type SampleTimer = ctimer::Ctimer2<init_state::Enabled>;
type ChargeMatch = Pin<ChargeMatchPin, Special<function::MATCH_OUTPUT3<ctimer::Ctimer1<init_state::Enabled>>>>;
type ButtonTop = Pin<ButtonTopPin, Analog<direction::Input>>;
type ButtonBot = Pin<ButtonBotPin, Analog<direction::Input>>;
type ButtonMid = Pin<ButtonMidPin, Analog<direction::Input>>;

pub type ThreeButtons = SoloThreeTouchButtons<ButtonTopPin, ButtonBotPin, ButtonMidPin>;

pub struct SoloThreeTouchButtons<P1,P2,P3>
where P1: PinId, P2: PinId, P3: PinId{
    touch_sensor: TouchSensor<P1,P2,P3>
}

impl SoloThreeTouchButtons<ButtonTopPin, ButtonBotPin, ButtonMidPin>
// where P1: PinId, P2: PinId, P3: PinId
{
    pub fn new (
        adc: Adc,
        adc_timer: AdcTimer,
        sample_timer: SampleTimer,
        charge_match: ChargeMatch,
        top: ButtonTop,
        bot: ButtonBot,
        mid: ButtonMid,
        dma: &mut Dma,
        token: ClocksSupportTouchToken,
    ) -> SoloThreeTouchButtons<ButtonTopPin, ButtonBotPin, ButtonMidPin> {
        let button_pins = ButtonPins(
            top,bot,mid,
        );
        let touch_sensor = TouchSensor::new([
            13_900,
            13_900,
            13_900,
            ], 5, adc, adc_timer, sample_timer, charge_match, button_pins);
        let touch_sensor = touch_sensor.enabled(dma, token);
        Self {
            touch_sensor
        }
    }

    /// Map internal cmd number to Button type
    fn button_get_state (&self, button: buttons::Button, ctype: Compare) -> bool {
        match button {
            Button::A => {
                self.touch_sensor.get_state(TouchSensorChannel::Channel1, ctype).is_active
            }
            Button::B => {
                self.touch_sensor.get_state(TouchSensorChannel::Channel2, ctype).is_active
            }
            Button::Middle => {
                self.touch_sensor.get_state(TouchSensorChannel::Channel3, ctype).is_active
            }
        }
    }

    /// Map internal cmd number to Button type
    fn button_has_edge (&self, button: Button, edge_type: Edge,) -> bool {
        match button {
            Button::A => {
                self.touch_sensor.has_edge(TouchSensorChannel::Channel1, edge_type)
            }
            Button::B => {
                self.touch_sensor.has_edge(TouchSensorChannel::Channel2, edge_type)
            }
            Button::Middle => {
                self.touch_sensor.has_edge(TouchSensorChannel::Channel3, edge_type)
            }

        }
    }

    fn button_reset_state(&self, button: Button, offset: i32) {
        match button {
            Button::A => {
                self.touch_sensor.reset_results(TouchSensorChannel::Channel1, offset);
            }
            Button::B => {
                self.touch_sensor.reset_results(TouchSensorChannel::Channel2, offset);
            }
            Button::Middle => {
                self.touch_sensor.reset_results(TouchSensorChannel::Channel3, offset);
            }
        }
    }


}

impl buttons::Press for SoloThreeTouchButtons<ButtonTopPin, ButtonBotPin, ButtonMidPin>
{
    fn is_pressed(&self, button: buttons::Button) -> bool {
        self.button_get_state(button, Compare::BelowThreshold)
    }

    fn is_released(&self, button: buttons::Button) -> bool {
        self.button_get_state(button, Compare::AboveThreshold)
    }
}

impl buttons::Edge for SoloThreeTouchButtons<ButtonTopPin, ButtonBotPin, ButtonMidPin>
{
    fn wait_for_new_press(&mut self, button: Button) -> nb::Result<(), Infallible> {
        let result = self.button_has_edge(button, Edge::Falling);

        // Erase edge with pressed status.
        if result {
            self.button_reset_state(button, -1);
            Ok(())
        } else {
            return Err(nb::Error::WouldBlock)
        }

    }

    fn wait_for_new_release(&mut self, button: Button) -> nb::Result<(), Infallible> {
        let result = self.button_has_edge(button, Edge::Rising);

        if result {
            self.button_reset_state(button, 1);
            Ok(())
        } else {
            return Err(nb::Error::WouldBlock)
        }

    }

    /// See wait_for_press
    fn wait_for_any_new_press(&mut self, ) -> nb::Result<Button, Infallible> {

        if self.wait_for_new_press(Button::A).is_ok() {
            Ok(Button::A)
        }
        else if self.wait_for_new_press(Button::B).is_ok() {
            Ok(Button::B)
        } else if self.wait_for_new_press(Button::Middle).is_ok() {
            Ok(Button::Middle)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// See wait_for_release
    fn wait_for_any_new_release(&mut self, ) -> nb::Result<Button, Infallible> {
        if self.wait_for_new_release(Button::A).is_ok() {
            Ok(Button::A)
        }
        else if self.wait_for_new_release(Button::B).is_ok() {
            Ok(Button::B)
        } else if self.wait_for_new_release(Button::Middle).is_ok() {
            Ok(Button::Middle)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// Wait for squeeze gesture
    fn wait_for_new_squeeze(&mut self) -> nb::Result<(), Infallible> {
        let a = self.button_has_edge(Button::A, Edge::Rising);
        let b = self.button_has_edge(Button::B, Edge::Rising);
        if a && b {
            self.button_reset_state(Button::A, -1);
            self.button_reset_state(Button::B, -1);
            Ok(())
        } else {
            return Err(nb::Error::WouldBlock)
        }
    }
}

