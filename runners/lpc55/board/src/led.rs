//! RGB LED driven by a ctimer's match outputs, shared by boards with hardware PWM.

use crate::hal::{
    drivers::pwm, peripherals::ctimer::Ctimer, traits::wg::Pwm, typestates::init_state,
};
use core::marker::PhantomData;

use crate::traits::rgb_led;

/// Per-board wiring of the RGB LED.
pub trait HwPwmConfig {
    /// Match-output channels for red, green, blue.
    const CHANNELS: [u8; 3];
    /// Divider applied to the timer's max duty.
    const MAX_DUTY_SCALE: u32;
    /// Scales a 0..255 intensity to timer duty, per channel.
    fn duty(channel: usize, intensity: u8) -> u16;
}

pub struct HwPwmLed<T, C>
where
    T: Ctimer<init_state::Enabled>,
    C: HwPwmConfig,
{
    pwm: pwm::Pwm<T>,
    _config: PhantomData<C>,
}

impl<T, C> HwPwmLed<T, C>
where
    T: Ctimer<init_state::Enabled>,
    C: HwPwmConfig,
{
    /// `into_outputs` converts the board's pins to match outputs. It runs after
    /// the duties are zeroed, so the pads never drive a stale duty.
    pub fn with_outputs<F: FnOnce()>(mut pwm: pwm::Pwm<T>, into_outputs: F) -> Self {
        for &channel in &C::CHANNELS {
            pwm.set_duty(channel, 0);
            pwm.enable(channel);
        }
        into_outputs();
        pwm.scale_max_duty_by(C::MAX_DUTY_SCALE);
        Self {
            pwm,
            _config: PhantomData,
        }
    }

    fn set(&mut self, channel: usize, intensity: u8) {
        self.pwm
            .set_duty(C::CHANNELS[channel], C::duty(channel, intensity));
    }
}

impl<T, C> rgb_led::RgbLed for HwPwmLed<T, C>
where
    T: Ctimer<init_state::Enabled>,
    C: HwPwmConfig,
{
    fn red(&mut self, intensity: u8) {
        self.set(0, intensity);
    }

    fn green(&mut self, intensity: u8) {
        self.set(1, intensity);
    }

    fn blue(&mut self, intensity: u8) {
        self.set(2, intensity);
    }
}
