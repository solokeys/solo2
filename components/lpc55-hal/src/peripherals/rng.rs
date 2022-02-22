use crate::{
    raw,
    peripherals::{
        syscon::Syscon,
    },
    typestates::{
        init_state,
    }
};

crate::wrap_stateful_peripheral!(Rng, RNG);

// /// HAL-ified RNG peripheral
// pub struct Rng<State = init_state::Enabled> {
//     raw: raw::RNG,
//     _state: State,
// }

#[derive(Debug)]
// not sure why this kind of thing is not in `svd2rust`?
pub struct ModuleId {
    id: u16,
    maj_rev: u8,
    min_rev: u8,
    aperture: u8,
}

impl Rng {
}

impl<State> Rng<State> {
    pub fn enabled(mut self, syscon: &mut Syscon) -> Rng<init_state::Enabled> {
        syscon.enable_clock(&mut self.raw);

        Rng {
            raw: self.raw,
            _state: init_state::Enabled(()),
        }
    }

    pub fn disabled(mut self, syscon: &mut Syscon) -> Rng<init_state::Disabled> {
        syscon.disable_clock(&mut self.raw);

        Rng {
            raw: self.raw,
            _state: init_state::Disabled,
        }
    }

}

impl Rng<init_state::Enabled> {
    /// DO NOT CALL - doesn't work yet
    #[allow(dead_code, unreachable_code)]
    fn initialize_entropy(&self) {
        unimplemented!();

        // NB: there are functional and operational differences between
        // the A0 and A1 versions of the chip, see UM 48.14 (page 1033)
        //
        // A0/A1 refer to syscon.dieid.rev
        //
        // Here, we assume A1 (as maj.min = 3.2 seems to indicate this)
        // TODO: check this is true for the lpcxpresso55s69
        // TODO: check again when going into production

        // poll ONLINE_TEST_VAL
        let val = &self.raw.online_test_val.read();
        #[allow(non_snake_case)]
        let REF_CHI_SQUARED = 2;

        // dbg!("shift4x is", self.raw.counter_cfg.read().shift4x().bits());
        // let _: u8 =  self.raw.counter_cfg.read().shift4x().bits();

        loop {
            // activate CHI computing
            // dbg!(self.raw.online_test_cfg.read().activate().bit());  // <-- false
            self.raw
                .online_test_cfg
                .modify(|_, w| unsafe { w.data_sel().bits(4) });
            self.raw
                .online_test_cfg
                .modify(|_, w| w.activate().set_bit());
            // dbg!(self.raw.online_test_cfg.read().activate().bit());  // <-- true

            // dbg!(val.min_chi_squared().bits());  // <-- 15
            // dbg!(val.max_chi_squared().bits());  // <--  0

            // TODO: this gets stuck
            // unimplemented!("figure out how to make this not block");
            while val.min_chi_squared().bits() > val.max_chi_squared().bits() {}

            // dbg!("passed");

            if val.max_chi_squared().bits() > REF_CHI_SQUARED {
                // reset
                self.raw
                    .online_test_cfg
                    .modify(|_, w| w.activate().clear_bit());
                // increment SHIFT4X, which has bit width 3
                // self.raw.counter_cfg.modify(|_, w| (w.shift4x().bits() as u8) + 1);
                continue;
            } else {
                break;
            }
        }
    }

    pub fn get_random_u32(&self) -> u32 {
        for _ in 0..32 {
            while self.raw.counter_val.read().refresh_cnt().bits() == 0 {
                // dbg!("was not zero");
            }
        }
        self.raw.random_number.read().bits()
    }

    /// random method to get some information about the RNG
    pub fn module_id(&self) -> ModuleId {
        ModuleId {
            id: self.raw.moduleid.read().id().bits(),
            maj_rev: self.raw.moduleid.read().maj_rev().bits(),
            min_rev: self.raw.moduleid.read().min_rev().bits(),
            aperture: self.raw.moduleid.read().aperture().bits(),
        }
    }
}

