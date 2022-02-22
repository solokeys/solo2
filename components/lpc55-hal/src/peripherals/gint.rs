use core::ops::Deref;

use crate::{
    peripherals::{
        syscon,
    },
    traits,
    typestates::{
        init_state,
    },
};

// Main struct
pub struct Gint<State: init_state::InitState = init_state::Unknown> {
    pub(crate) raw_gint0: raw::GINT0,
    pub(crate) raw_gint1: raw::GINT1,
    _state: State,
}

impl Gint {
    pub fn new(raw_gint0: raw::GINT0, raw_gint1: raw::GINT1) -> Self {
        Gint {
            raw_gint0,
            raw_gint1,
            _state: init_state::Unknown,
        }
    }
}

// do a little macro here

pub struct Gint0<State: init_state::InitState = init_state::Enabled> {
    pub(crate) raw: raw::GINT0,
    _state: State,
}
impl Deref for Gint0 {
    type Target = raw::gint0::RegisterBlock;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}
impl traits::Gint for Gint0 {}

pub type EnabledGint0 = Gint0<init_state::Enabled>;

pub struct Gint1<State: init_state::InitState = init_state::Enabled> {
    pub(crate) raw: raw::GINT1,
    _state: State,
}
impl Deref for Gint1 {
    type Target = raw::gint0::RegisterBlock;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}
impl traits::Gint for Gint1 {}

pub type EnabledGint1 = Gint1<init_state::Enabled>;

impl<State: init_state::InitState> Gint<State> {
    pub fn release(self) -> (raw::GINT0, raw::GINT1) {
        (self.raw_gint0, self.raw_gint1)
    }

    pub fn enabled(
        mut self,
        syscon: &mut syscon::Syscon,
    ) -> (Gint0, Gint1) {
        syscon.enable_clock(&mut (&mut self.raw_gint0, &mut self.raw_gint1));
        (
            Gint0 {
                raw: self.raw_gint0,
                _state: init_state::Enabled(()),
            },
            Gint1 {
                raw: self.raw_gint1,
                _state: init_state::Enabled(()),
            },
        )
    }
}

// impl (Gint0, Gint1) {
//     pub fn disabled(
//         mut self,
//         syscon: &mut syscon::Syscon,
//     ) -> Gint<init_state::Disabled> {
//         syscon.disable_clock(&mut self);

//         Gint {
//             raw_gint0: self.0.raw,
//             raw_gint1: self.1.raw,
//             _state: init_state::Disabled,
//         }
//     }
// }

impl From<(raw::GINT0, raw::GINT1)> for Gint {
    fn from(raw: (raw::GINT0, raw::GINT1)) -> Self {
        Gint::new(raw.0, raw.1)
    }
}
