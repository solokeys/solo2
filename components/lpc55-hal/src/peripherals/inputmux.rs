use crate::{
    raw,
    peripherals::{
        syscon,
    },
    typestates::{
        init_state,
    }
};

crate::wrap_stateful_peripheral!(InputMux, INPUTMUX);

impl<State> InputMux<State> {
    pub fn enabled(mut self, syscon: &mut syscon::Syscon) -> InputMux<init_state::Enabled> {
        syscon.enable_clock(&mut self.raw);

        InputMux {
            raw: self.raw,
            _state: init_state::Enabled(()),
        }
    }

    pub fn disabled(mut self, syscon: &mut syscon::Syscon) -> InputMux<init_state::Disabled> {
        syscon.disable_clock(&mut self.raw);

        InputMux {
            raw: self.raw,
            _state: init_state::Disabled,
        }
    }

}

