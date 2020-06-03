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
use solo_bee_traits::buttons::{
    self,
    ButtonPress,
    ButtonEdge,
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
    fn button_get_state (&self, button: buttons::Button, ctype: Compare) -> buttons::Button {
        match button {
            buttons::ButtonTop => {
                if self.touch_sensor.get_state(TouchSensorChannel::Channel1, ctype).is_active { return button };
            }
            buttons::ButtonBot => {
                if self.touch_sensor.get_state(TouchSensorChannel::Channel2, ctype).is_active { return button };
            }
            buttons::ButtonSides => {
                if self.touch_sensor.get_state(TouchSensorChannel::Channel2, ctype).is_active &&
                    self.touch_sensor.get_state(TouchSensorChannel::Channel1, ctype).is_active
                        { return button };
            }
            buttons::ButtonMid=> {
                if self.touch_sensor.get_state(TouchSensorChannel::Channel3, ctype).is_active  { return button };
            }
            buttons::ButtonAny => {
                if self.touch_sensor.get_state(TouchSensorChannel::Channel3, ctype).is_active { return buttons::ButtonMid };
                if self.touch_sensor.get_state(TouchSensorChannel::Channel2, ctype).is_active { return buttons::ButtonBot};
                if self.touch_sensor.get_state(TouchSensorChannel::Channel1, ctype).is_active { return buttons::ButtonTop};
            }
            buttons::ButtonNone => {
                return button;
            }
        }
        return buttons::ButtonNone;
    }

    /// Map internal cmd number to Button type
    fn button_has_edge (&self, button: Button, edge_type: Edge,) -> buttons::Button {
        match button {
            buttons::ButtonTop => {
                if self.touch_sensor.has_edge(TouchSensorChannel::Channel1, edge_type) { return button }
            }
            buttons::ButtonBot => {
                if self.touch_sensor.has_edge(TouchSensorChannel::Channel2, edge_type) { return button }
            }
            buttons::ButtonSides => {
                if
                    self.touch_sensor.has_edge(TouchSensorChannel::Channel1, edge_type) &&
                    self.touch_sensor.has_edge(TouchSensorChannel::Channel2, edge_type)
                        { return button }
            }
            buttons::ButtonMid=> {
                if
                    self.touch_sensor.has_edge(TouchSensorChannel::Channel3, edge_type)
                        {  return button }
            }
            buttons::ButtonAny => {
                if self.touch_sensor.has_edge(TouchSensorChannel::Channel1, edge_type) {return buttons::ButtonTop}
                if self.touch_sensor.has_edge(TouchSensorChannel::Channel2, edge_type) {return buttons::ButtonBot}
                if self.touch_sensor.has_edge(TouchSensorChannel::Channel3, edge_type) {return buttons::ButtonMid}
            }
            buttons::ButtonNone => {}
        }

        buttons::ButtonNone
    }

    fn button_reset_state(&self, button: Button, offset: i32) {
        match button {
            buttons::ButtonTop => {
                self.touch_sensor.reset_results(TouchSensorChannel::Channel1, offset);
            }
            buttons::ButtonBot => {
                self.touch_sensor.reset_results(TouchSensorChannel::Channel2, offset);
            }
            buttons::ButtonSides => {
                self.touch_sensor.reset_results(TouchSensorChannel::Channel2, offset);
                self.touch_sensor.reset_results(TouchSensorChannel::Channel1, offset);
            }
            buttons::ButtonMid=> {
                self.touch_sensor.reset_results(TouchSensorChannel::Channel3, offset);
            }
            _ => {
                panic!("Invaid button combination to reset")
            }
        }
    }


}

impl ButtonPress for SoloThreeTouchButtons<ButtonTopPin, ButtonBotPin, ButtonMidPin>
{
    fn is_pressed(&self, button: buttons::Button) -> bool {
        self.button_get_state(button, Compare::BelowThreshold) != buttons::ButtonNone
    }

    fn is_released(&self, button: buttons::Button) -> bool {
        self.button_get_state(button, Compare::AboveThreshold) != buttons::ButtonNone
    }


    fn get_status(&self) -> buttons::Buttons {
        buttons::Buttons {
            top: self.is_pressed(buttons::ButtonTop),
            bot: self.is_pressed(buttons::ButtonBot),
            mid: self.is_pressed(buttons::ButtonMid),
        }
    }

}

impl ButtonEdge for SoloThreeTouchButtons<ButtonTopPin, ButtonBotPin, ButtonMidPin>
{
    fn wait_for_press(&mut self, button: buttons::Button) -> nb::Result<Button, Infallible> {
        let button = self.button_has_edge(button, Edge::Falling);

        // Erase edge with pressed status.
        if button != buttons::ButtonNone {
            self.button_reset_state(button, -1);
        } else {
            return Err(nb::Error::WouldBlock);
        }

        Ok(button)
    }

    fn wait_for_release(&mut self, button: buttons::Button) -> nb::Result<Button, Infallible> {
        let button = self.button_has_edge(button, Edge::Rising);

        // Erase edge with released status.
        if button != buttons::ButtonNone {
            self.button_reset_state(button, 1);
        } else {
            return Err(nb::Error::WouldBlock);
        }

        Ok(button)
    }

    /// See wait_for_press
    fn wait_for_any_press(&mut self, ) -> nb::Result<Button, Infallible> {
        self.wait_for_press(buttons::ButtonAny)
    }

    /// See wait_for_release
    fn wait_for_any_release(&mut self, ) -> nb::Result<Button, Infallible> {
        self.wait_for_release(buttons::ButtonAny)
    }
}

