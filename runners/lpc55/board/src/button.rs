//! Charge-transfer touch buttons, shared by every board that has them.

use crate::hal::{
    self,
    drivers::pins,
    drivers::touch::{ButtonPins, Compare, Edge as TouchEdge, TouchSensor, TouchSensorChannel},
    peripherals::ctimer,
    typestates::{init_state, pin::PinId, ClocksSupportTouchToken},
};
use core::convert::Infallible;
use core::marker::PhantomData;

use crate::traits::buttons::{self, Button};

pub type ChargeMatchPin = pins::Pio1_16;

type Adc = hal::peripherals::adc::Adc<init_state::Enabled>;
type Dma = hal::peripherals::dma::Dma<init_state::Enabled>;
type AdcTimer = ctimer::Ctimer1<init_state::Enabled>;
type SampleTimer = ctimer::Ctimer2<init_state::Enabled>;

/// Per-board wiring of the touch buttons.
pub trait ButtonConfig {
    type ButtonA: PinId;
    type ButtonB: PinId;
    type ButtonMiddle: PinId;
    const THRESHOLDS: [u32; 3];
    const CONFIDENCE: u32;
    /// Buttons the `wait_for_any_*` gestures scan, in order.
    const SCAN: &'static [Button];
    fn channel(button: Button) -> TouchSensorChannel;
}

pub struct TouchButtons<C: ButtonConfig> {
    sensor: TouchSensor<C::ButtonA, C::ButtonB, C::ButtonMiddle>,
    _config: PhantomData<C>,
}

impl<C: ButtonConfig> TouchButtons<C> {
    /// Boards convert their own buttons: `into_analog_input` is an inherent method
    /// on each pin type, so it cannot be called generically.
    pub fn from_pins(
        adc: Adc,
        adc_timer: AdcTimer,
        sample_timer: SampleTimer,
        dma: &mut Dma,
        token: ClocksSupportTouchToken,
        iocon: &mut hal::Iocon<hal::Enabled>,
        buttons: ButtonPins<C::ButtonA, C::ButtonB, C::ButtonMiddle>,
    ) -> Self {
        let charge_match = ChargeMatchPin::take().unwrap().into_match_output(iocon);
        let sensor = TouchSensor::new(
            C::THRESHOLDS,
            C::CONFIDENCE,
            adc,
            adc_timer,
            sample_timer,
            charge_match,
            buttons,
        );
        Self {
            sensor: sensor.enabled(dma, token),
            _config: PhantomData,
        }
    }

    fn state(&self, button: Button, ctype: Compare) -> bool {
        self.sensor.get_state(C::channel(button), ctype).is_active
    }

    fn edge(&self, button: Button, edge: TouchEdge) -> bool {
        self.sensor.has_edge(C::channel(button), edge)
    }

    fn reset(&self, button: Button, offset: i32) {
        self.sensor.reset_results(C::channel(button), offset);
    }
}

impl<C: ButtonConfig> buttons::Press for TouchButtons<C> {
    fn is_pressed(&self, button: Button) -> bool {
        self.state(button, Compare::BelowThreshold)
    }

    fn is_released(&self, button: Button) -> bool {
        self.state(button, Compare::AboveThreshold)
    }
}

impl<C: ButtonConfig> buttons::Edge for TouchButtons<C> {
    fn wait_for_new_press(&mut self, button: Button) -> nb::Result<(), Infallible> {
        if self.edge(button, TouchEdge::Falling) {
            self.reset(button, -1);
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    fn wait_for_new_release(&mut self, button: Button) -> nb::Result<(), Infallible> {
        if self.edge(button, TouchEdge::Rising) {
            self.reset(button, 1);
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    fn wait_for_any_new_press(&mut self) -> nb::Result<Button, Infallible> {
        for &button in C::SCAN {
            if self.wait_for_new_press(button).is_ok() {
                return Ok(button);
            }
        }
        Err(nb::Error::WouldBlock)
    }

    fn wait_for_any_new_release(&mut self) -> nb::Result<Button, Infallible> {
        for &button in C::SCAN {
            if self.wait_for_new_release(button).is_ok() {
                return Ok(button);
            }
        }
        Err(nb::Error::WouldBlock)
    }

    fn wait_for_new_squeeze(&mut self) -> nb::Result<(), Infallible> {
        let a = self.edge(Button::A, TouchEdge::Rising);
        let b = self.edge(Button::B, TouchEdge::Rising);
        if a && b {
            self.reset(Button::A, -1);
            self.reset(Button::B, -1);
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}
