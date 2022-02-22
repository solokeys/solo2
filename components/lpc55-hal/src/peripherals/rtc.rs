use core::time::Duration;
use crate::{
    raw,
    peripherals::{
        syscon::Syscon,
    },
    typestates::{
        init_state,
        ClocksSupport32KhzFroToken,
    }
};

crate::wrap_stateful_peripheral!(Rtc, RTC);

impl<State> Rtc<State> {
    pub fn enabled(mut self, syscon: &mut Syscon, _token: ClocksSupport32KhzFroToken) -> Rtc<init_state::Enabled> {
        syscon.enable_clock(&mut self.raw);
        self.raw.ctrl.write(|w| 
            w
            .rtc_en().set_bit()
            .rtc_subsec_ena().set_bit()
            .swreset().clear_bit()
            .rtc_osc_pd().clear_bit()
        );
        Rtc {
            raw: self.raw,
            _state: init_state::Enabled(()),
        }
    }

    pub fn disabled(mut self, syscon: &mut Syscon) -> Rtc<init_state::Disabled> {
        syscon.disable_clock(&mut self.raw);

        Rtc {
            raw: self.raw,
            _state: init_state::Disabled,
        }
    }
}

impl Rtc<init_state::Enabled> {
    pub fn uptime(&self) -> Duration {
        let secs = self.raw.count.read().bits() as u64;
        let ticks_32k = self.raw.subsec.read().bits() as u64;
        Duration::from_secs(secs) + Duration::from_micros((ticks_32k * 61)/2)
    }

    pub fn reset(&mut self) {
        self.raw.ctrl.write(|w| w.swreset().set_bit() );
        while self.raw.ctrl.read().swreset().is_not_in_reset() {}
        self.raw.ctrl.write(|w| w.swreset().clear_bit() );
        while self.raw.ctrl.read().swreset().is_in_reset() {}
        self.raw.ctrl.write(|w| 
            w
            .rtc_en().set_bit()
            .swreset().clear_bit()
            .rtc_osc_pd().clear_bit()
        );
        // After reset:
        // This bit can only be set after the RTC_ENA bit (bit 7) is set by a previous write operation.
        self.raw.ctrl.modify(|_,w| w.rtc_subsec_ena().set_bit() )
    }
}