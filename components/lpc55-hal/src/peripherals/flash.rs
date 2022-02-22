use crate::{
    raw,
    peripherals::{
        syscon,
    },
    typestates::{
        init_state,
    }
};

crate::wrap_stateful_peripheral!(Flash, FLASH);

impl<State> Flash<State> {
    pub fn enabled(mut self, syscon: &mut syscon::Syscon) -> Flash<init_state::Enabled> {
        syscon.enable_clock(&mut self.raw);

        Flash {
            raw: self.raw,
            _state: init_state::Enabled(()),
        }
    }

    pub fn disabled(mut self, syscon: &mut syscon::Syscon) -> Flash<init_state::Disabled> {
        syscon.disable_clock(&mut self.raw);

        Flash {
            raw: self.raw,
            _state: init_state::Disabled,
        }
    }

}
