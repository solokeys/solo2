use crate::{
    raw,
    typestates::init_state,
    peripherals::syscon,
};

crate::wrap_stateful_peripheral!(Gpio, GPIO);

impl Gpio {
    /// Consumes disabled Gpio, returns an enabled one
    pub fn enabled(mut self, syscon: &mut syscon::Syscon) -> Gpio<init_state::Enabled> {
        // dbg!(syscon.is_clock_enabled(&self.gpio));
        syscon.enable_clock(&mut self.raw);
        // dbg!(syscon.is_clock_enabled(&self.gpio));

        Gpio {
            raw: self.raw,
            _state: init_state::Enabled(()),
        }
    }

    /// Consumes enabled Gpio, returns a disabled one
    pub fn disabled(mut self, syscon: &mut syscon::Syscon) -> Gpio<init_state::Disabled> {
        syscon.disable_clock(&mut self.raw);

        Gpio {
            raw: self.raw,
            _state: init_state::Disabled,
        }
    }
}
