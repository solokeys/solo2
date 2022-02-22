use core::ops::Deref;
use crate::{
    raw,
    peripherals::{
        syscon::Syscon,
    },
    typestates::{
        init_state,
        ClocksSupport1MhzFroToken,
    },
};


pub type Ctimers = (
    Ctimer0,
    Ctimer1,
    Ctimer2,
    Ctimer3,
    Ctimer4,
);

pub trait Ctimer<State>: Deref<Target = raw::ctimer0::RegisterBlock> {}

macro_rules! ctimer {
    ($c_hal:ident, $c_pac:ident, $register:ident, $clock_input:ident) => {

    crate::wrap_stateful_peripheral!($c_hal, $c_pac);

    impl Deref for $c_hal<init_state::Enabled> {
        type Target = raw::ctimer0::RegisterBlock;
        fn deref(&self) -> &Self::Target {
            &self.raw
        }
    }
    impl Ctimer<init_state::Enabled> for $c_hal<init_state::Enabled> {}


    impl<State> $c_hal<State> {
        pub fn enabled(mut self, syscon: &mut Syscon, _token: ClocksSupport1MhzFroToken) -> $c_hal <init_state::Enabled> {
            syscon.enable_clock(&mut self.raw);
            syscon.raw.$register().write(|w| { w.sel().$clock_input() } );
            syscon.reset(&mut self.raw);
            $c_hal {
                raw: self.raw,
                _state: init_state::Enabled(()),
            }
        }

        pub fn disabled(mut self, syscon: &mut Syscon) -> $c_hal <init_state::Disabled> {
            syscon.disable_clock(&mut self.raw);
            syscon.raw.$register().write(|w| { w.sel().enum_0x7() } );  // no clock
            $c_hal {
                raw: self.raw,
                _state: init_state::Disabled,
            }
        }
    }



    }
}

ctimer!(Ctimer0, CTIMER0, ctimerclksel0, enum_0x4);    // 4 is 1MHz FRO
ctimer!(Ctimer1, CTIMER1, ctimerclksel1, enum_0x4);
ctimer!(Ctimer2, CTIMER2, ctimerclksel2, enum_0x4);
ctimer!(Ctimer3, CTIMER3, ctimerclksel3, enum_0x4);
ctimer!(Ctimer4, CTIMER4, ctimerclksel4, enum_0x4);
