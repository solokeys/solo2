//! API for the micro-tick timer (UTICK)
//!
//! The entry point to this API is [`UTICK`].
//!
//! The UTICK peripheral is described in the user manual, chapter 26.
//! It is driven by the FRO 1Mhz clock and has a microsecond resolution.
//!
//! # Examples: led.rs, led_utick.rs

// TODO: move this to drivers section,
// possibly merge with ctimers when they're implemented

use core::convert::Infallible;
use embedded_hal::timer;
use nb;
use void::Void;

use crate::{
    raw,
    peripherals::{
        syscon,
    },
    typestates::{
        init_state,
        ClocksSupportUtickToken,
    },
};

crate::wrap_stateful_peripheral!(Utick, UTICK0);

pub type EnabledUtick = Utick<init_state::Enabled>;

impl<State> Utick<State> {
    pub fn enabled(
        mut self,
        syscon: &mut syscon::Syscon,
        _clocktree_token: &ClocksSupportUtickToken,
    ) -> EnabledUtick {
        syscon.enable_clock(&mut self.raw);
        syscon.reset(&mut self.raw);

        Utick {
            raw: self.raw,
            _state: init_state::Enabled(()),
        }
    }

    pub fn disabled(mut self, syscon: &mut syscon::Syscon) -> Utick<init_state::Disabled> {
        syscon.disable_clock(&mut self.raw);

        Utick {
            raw: self.raw,
            _state: init_state::Disabled,
        }
    }
}

// TODO: This does not feel like it belongs here.

impl timer::Cancel for EnabledUtick {
    type Error = Infallible;

    fn cancel(&mut self) -> Result<(), Self::Error> {
        // A value of 0 stops the timer.
        self.raw.ctrl.write(|w| unsafe { w.delayval().bits(0) });
        Ok(())
    }
}

// TODO: also implement Periodic for UTICK
impl timer::CountDown for EnabledUtick {
    type Time = u32;

    fn start<T>(&mut self, timeout: T)
    where
        T: Into<Self::Time>,
    {
        // The delay will be equal to DELAYVAL + 1 periods of the timer clock.
        // The minimum usable value is 1, for a delay of 2 timer clocks. A value of 0 stops the timer.
        let time = timeout.into();
        // Maybe remove again? Empirically, nothing much happens when
        // writing 1 to `delayval`.
        assert!(time >= 2);
        self.raw
            .ctrl
            .write(|w| unsafe { w.delayval().bits(time - 1) });
        // So... this seems a bit unsafe (what if time is 2?)
        // But: without it, in --release builds the timer behaves erratically.
        // The UM says this on the topic: "Note that the Micro-tick Timer operates from a different
        // (typically slower) clock than the CPU and bus systems.  This means there may be a
        // synchronization delay when accessing Micro-tick Timer registers."
        while self.raw.stat.read().active().bit_is_clear() {}
    }

    fn wait(&mut self) -> nb::Result<(), Void> {
        if self.raw.stat.read().active().bit_is_clear() {
            return Ok(());
        }

        Err(nb::Error::WouldBlock)
    }
}

// TODO: Either get rid of `nb` or get rid of this
impl EnabledUtick {
    pub fn blocking_wait(&mut self) {
        while self.raw.stat.read().active().bit_is_set() {}
    }
}
