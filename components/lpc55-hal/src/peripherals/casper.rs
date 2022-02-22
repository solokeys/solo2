use crate::{
    raw,
    peripherals::{
        syscon,
    },
    typestates::{
        init_state,
    }
};

crate::wrap_stateful_peripheral!(Casper, CASPER);

impl<State> Casper<State> {
    pub fn enabled(
        mut self,
        syscon: &mut syscon::Syscon,
    ) -> Casper<init_state::Enabled> {
        syscon.enable_clock(&mut self.raw);
        syscon.reset(&mut self.raw);
        Casper {
            raw: self.raw,
            _state: init_state::Enabled(()),
        }
    }

    pub fn disabled(
        mut self,
        syscon: &mut syscon::Syscon,
    ) -> Casper<init_state::Disabled> {
        syscon.disable_clock(&mut self.raw);
        Casper {
            raw: self.raw,
            _state: init_state::Disabled,
        }
    }
}

pub enum Operations {
    /// Walking 1 or more of J loop, doing r=a*b using 64x64=128
    Mul6464NoSum = 0x01,
    /// Walking 1 or more of J loop, doing c,r=r+a*b using 64x64=128, but assume inner j loop
    Mul6464Sum = 0x02,
    /// Walking 1 or more of J loop, doing c,r=r+a*b using 64x64=128, but sum all of w.
    Mul6464FullSum = 0x03,
    /// Walking 1 or more of J loop, doing c,r[-1]=r+a*b using 64x64=128, but skip 1st write
    Mul6464Reduce = 0x04,
    /// Walking add with off_AB, and in/out off_RES doing c,r=r+a+c using 64+64=65
    Add64 = 0x08,
    /// Walking subtract with off_AB, and in/out off_RES doing r=r-a uding 64-64=64,
    /// with last borrow implicit if any
    Sub64 = 0x09,
    /// Walking add to self with off_RES doing c,r=r+r+c using 64+64=65
    Double64 = 0x0A,
    /// Walking XOR with off_AB, and in/out off_RES doing r=r^a using 64^64=64
    Xor64 = 0x0B,
    /// Walking shift left doing r1,r=(b*D)|r1, where D is 2^amt and is loaded
    /// by app (off_CD not used)
    ShiftLeft32 = 0x10,
    /// Walking shift right doing r,r1=(b*D)|r1, where D is 2^(32-amt) and is loaded
    /// by app (off_CD not used) and off_RES starts at MSW
    ShiftRight32 = 0x11,
    /// Copy from ABoff to resoff, 64b at a time
    Copy = 0x14,
    /// Copy and mask from ABoff to resoff, 64b at a time
    Remask = 0x15,
    /// Compare two arrays, running all the way to the end
    Compare = 0x16,
    /// Compare two arrays, stopping on 1st !=^
    CompareFast = 0x17,
}
