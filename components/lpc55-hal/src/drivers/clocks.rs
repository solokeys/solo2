///!* API to configure the clocks.
///!
///! This is very incomplete (e.g., no support for PLL clocks).
///! It is also likely buggy, and more complex than needed
///!
///! It is currently used to prepare for using the USBFSD and
///! Flexcomm peripherals.
use core::{cmp::min, convert::TryFrom};
use embedded_time::rate::Extensions;

use crate::typestates::{
    main_clock::MainClock,
    // clock_state,
    ClocksSupportFlexcommToken,
    ClocksSupportUsbfsToken,
    ClocksSupportUsbhsToken,
    ClocksSupportUtickToken,
    ClocksSupportTouchToken,
    ClocksSupport1MhzFroToken,
    ClocksSupport32KhzFroToken,
};
use crate::{
    peripherals::{
        anactrl::Anactrl,
        pmc::Pmc,
        syscon::Syscon,
    },
    time::{Hertz, Megahertz},
};

// #[allow(unused_imports)]
// use cortex_m_semihosting::{hprintln, dbg};

// UM 41.4.7 says: need >= 12Mhz for USBFS
// Empirically, this does not enumerate though
// TODO: It seems even Fro12Mhz works, but e.g.
// PLL at 13.mhz does not - might also be a bug
// in PLL code ofc
const MIN_USBFS_FREQ: Megahertz = Megahertz(24);
const MIN_USBHS_FREQ: Megahertz = Megahertz(96);
const DEFAULT_FREQ: Megahertz = Megahertz(12);

#[derive(Debug, Default)]
pub struct ClockRequirements {
    pub system_frequency: Option<Megahertz>,
    pub custom_pll: Option<Pll>,
}

#[derive(Debug, Copy, Clone)]
pub struct Clocks {
    pub(crate) main_clock: MainClock,
    pub(crate) system_frequency: Hertz,
}

impl Clocks {
    pub fn support_flexcomm_token(&self) -> Option<ClocksSupportFlexcommToken> {
        Some(ClocksSupportFlexcommToken{__: ()})
    }

    pub fn support_usbfs_token(&self) -> Option<ClocksSupportUsbfsToken> {
        let fast_enough = self.system_frequency >= Hertz::from(MIN_USBFS_FREQ);
        let can_latch_sof = self.main_clock == MainClock::Fro96Mhz;

        if fast_enough && can_latch_sof {
            Some(ClocksSupportUsbfsToken{__: ()})
        } else {
            None
        }
    }

    pub fn support_usbhs_token(&self) -> Option<ClocksSupportUsbhsToken> {
        let fast_enough = self.system_frequency >= Hertz::from(MIN_USBHS_FREQ);
        if fast_enough {
            Some(ClocksSupportUsbhsToken{__: ()})
        } else {
            None
        }
    }

    pub fn support_utick_token(&self) -> Option<ClocksSupportUtickToken> {
        Some(ClocksSupportUtickToken{__: ()})
    }

    pub fn support_1mhz_fro_token(&self) -> Option<ClocksSupport1MhzFroToken> {
        Some(ClocksSupport1MhzFroToken{__: ()})
    }

    pub fn support_touch_token(&self) -> Option<ClocksSupportTouchToken> {
        if self.system_frequency.0 >= 96 {
            Some(ClocksSupportTouchToken{__: ()})
        } else {
            None
        }
    }

    pub fn enable_32k_fro(&self, pmc: &mut Pmc) -> ClocksSupport32KhzFroToken {
        let mut token = ClocksSupport32KhzFroToken{__: ()};
        pmc.power_on(&mut token);
        token
    }

}

/// Output of Pll is: M/(2NP) times input
///
/// "There may be several ways to obtain the same PLL output frequency.
/// PLL power depends on Fcco (a lower frequency uses less power) and the divider used.
/// Bypassing the input and/or output divider saves power."

// #[allow(dead_code)]
#[derive(Debug)]
pub struct Pll {
    n: u8,
    m: u16,
    p: u8,
    selp: u8,
    seli: u8,
}

impl Pll {
    // allow user to override if they know better...
    pub unsafe fn new(n: u8, m: u16, p: u8) -> Pll {
        // UM 4.6.6.3.2
        let selp = min((m >> 2) + 1, 31) as u8;
        let seli = min(63, match m {
            m if m >= 8000 => 1,
            m if m >= 122 => 8000 / m,
            _ => 2 * (m >> 2) + 3,
        }) as u8;
        // let seli = min(2*(m >> 2) + 3, 63);
        Pll { n, m, p, selp, seli }
    }
}


static mut CONFIGURED: bool = false;

#[derive(Debug)]
pub enum ClocksError {
    // TODO: Add "cause"
    AlreadyConfigured,
    NotFeasible,
}

pub type Result<T> = core::result::Result<T, ClocksError>;

// TODO:
// - make sure Fro12Mhz is running for FLEXCOMM0
// - make sure Fro12 and Fro96 are even powered

impl ClockRequirements {

    pub fn system_frequency<Freq>(mut self, freq: Freq) -> Self where Freq: Into<Megahertz> {
        self.system_frequency = Some(freq.into());
        self
    }

    // generated via `scripts/generate-pll-settings.py`,
    // massaged a bit by hand
    fn get_pll(freq: u32) -> Pll {
        debug_assert!(freq >= 5);
        debug_assert!(freq <= 150);
        // let ns: [u32; 9] = [1, 2, 3, 4, 6, 8, 12, 16, 24];
        // let ns: [u32; 2] = [1, 2];
        // let ps: [u32; 11] = [2, 3, 4, 6, 8, 9, 12, 16, 18, 24, 30];
        // let ps: [u32; 5] = [6, 8, 12, 16, 24];

        // for n in ns.iter() { for p in ps.iter() { for m in 3..=97 {
        for n in 1..=6 { for p in 1..=30 { for m in 1..=255 {
            // if 2 * freq * (*n) * (*p) == 12 * m {
            if 2 * freq * n * p == 12 * m {
                // UM 4.6.6.3.2
                let selp = (m >> 2) + 1; // <= 31
                let seli = 2 * (m >> 2) + 3; // <= 63
                return Pll {
                    // n: *n as u8,
                    n: n as u8,
                    m: m as u16,
                    // p: *p as u8,
                    p: p as u8,
                    selp: selp as u8,
                    seli: seli as u8,
                }
            }
        }}}

        unreachable!();
    }

    fn configure_pll0(pll: Pll, pmc: &mut Pmc, syscon: &mut Syscon) {

        pmc.raw.pdruncfg0.modify(|_, w| w
            .pden_pll0().poweredoff()
            .pden_pll0_sscg().poweredoff()
        );

        syscon.raw.pll0ctrl.write(|w| unsafe { w
            .clken().enable()
            .seli().bits(pll.seli)
            .selp().bits(pll.selp)
        });

        syscon.raw.pll0ndec.write(|w| unsafe { w
            .ndiv().bits(pll.n)
        });
        syscon.raw.pll0ndec.write(|w| unsafe { w
            .ndiv().bits(pll.n)
            .nreq().set_bit() // latch
        });

        syscon.raw.pll0pdec.write(|w| unsafe { w
            .pdiv().bits(pll.p)
        });
        syscon.raw.pll0pdec.write(|w| unsafe { w
            .pdiv().bits(pll.p)
            .preq().set_bit() // latch
        });

        syscon.raw.pll0sscg0.write(|w| unsafe { w
            .md_lbs().bits(0)
        });

        syscon.raw.pll0sscg1.write(|w| unsafe { w
            .mdiv_ext().bits(pll.m)
            .sel_ext().set_bit()
        });
        syscon.raw.pll0sscg1.write(|w| unsafe { w
            .mdiv_ext().bits(pll.m)
            .sel_ext().set_bit()
            .mreq().set_bit() // latch
            .md_req().set_bit() // latch
        });

        pmc.raw.pdruncfg0.modify(|_, w| w
            .pden_pll0().poweredon()
            .pden_pll0_sscg().poweredon()
        );

        // wait at least 6 ms for PLL to stabilize
        crate::wait_at_least(6_000);
    }

    fn get_clock_source_and_div_for_freq(freq: Megahertz, pmc: &mut Pmc, syscon: &mut Syscon) -> (MainClock, u8) {
        let (main_clock, sys_divider) = match freq {
            freq if freq <= 12_u32.MHz() && 12 % freq.0 == 0 => {
                (MainClock::Fro12Mhz, 12 / freq.0)
            },
            freq if freq <= 96_u32.MHz() && 96 % freq.0 == 0 => {
                (MainClock::Fro96Mhz, 96 / freq.0)
            },
            // For reference: how to get 150 MHz using 16Mhz external crystal
            // freq if freq == 150.mhz() && 150 % freq.0 == 0 => {
            //     // Use crystal as input to PLL0
            //     // Power on 32M crystal for stable pll operation
            //     pmc.raw.pdruncfg0.modify(|_,w| w.pden_xtal32m().poweredon());
            //     pmc.raw.pdruncfg0.modify(|_,w| w.pden_ldoxo32m().poweredon());

            //     // Connect external 32M as clk input
            //     syscon.raw.clock_ctrl.modify(|_,w| w.clkin_ena().set_bit());
            //     anactrl.raw.xo32m_ctrl.modify(|_,w| w.enable_system_clk_out().set_bit());

            //     // select clkin for pll0
            //     syscon.raw.pll0clksel.write(|w| unsafe{ w.bits(1) });

            //     // 150 MHz settings
            //     let pll = Pll {
            //         n: 8,
            //         m: 150,
            //         p: 1,
            //         selp: 31,
            //         seli: 53,
            //     };

            //     Self::configure_pll0(pll, pmc, syscon);

            //     (MainClock::Pll0, 1)
            // }
            // Get 150 MHz using internal FRO12
            freq if freq == 150_u32.MHz() => {
                syscon.raw.pll0clksel.write(|w| { w.sel().enum_0x0() /* FRO 12 MHz input */ });
                Self::configure_pll0(Pll {
                    n: 8,
                    m: 200,
                    p: 1,
                    selp: 31,
                    seli: 53,
                }, pmc, syscon);
                (MainClock::Pll0, 1)
            }

            _ => {
                let pll = Self::get_pll(freq.0);
                syscon.raw.pll0clksel.write(|w| { w.sel().enum_0x0() /* FRO 12 MHz input */ });
                Self::configure_pll0(pll, pmc, syscon);
                (MainClock::Pll0, 1)
            }
        };
        debug_assert!(sys_divider < 256);
        (main_clock, sys_divider as u8)
    }

    fn set_new_clock_source(freq: Megahertz, main_clock: MainClock, sys_divider: u8, syscon: &mut Syscon) {
        // set highest flash wait cycles
        syscon.raw.fmccr.modify(|_, w| unsafe{ w.flashtim().bits(11) });

        match main_clock {
            MainClock::Fro12Mhz => {
                // Fro12
                syscon.raw.mainclksela.modify(|_, w| w.sel().enum_0x0());
                // Main A
                syscon.raw.mainclkselb.modify(|_, w| w.sel().enum_0x0());
                unsafe { syscon.raw.ahbclkdiv.modify(|_, w| w.div().bits(sys_divider - 1)) };
            },
            MainClock::Fro96Mhz => {
                // Fro96
                syscon.raw.mainclksela.modify(|_, w| w.sel().enum_0x3());
                // Main B
                syscon.raw.mainclkselb.modify(|_, w| w.sel().enum_0x0());
                unsafe { syscon.raw.ahbclkdiv.modify(|_, w| w.div().bits(sys_divider - 1)) };
            },
            MainClock::Pll0 => {
                // Fro12
                syscon.raw.mainclksela.modify(|_, w| w.sel().enum_0x0());
                // Pll0
                syscon.raw.mainclkselb.modify(|_, w| w.sel().enum_0x1());
                unsafe { syscon.raw.ahbclkdiv.modify(|_, w| w.div().bits(sys_divider - 1)) };

            }
        }

        // fix wait cycles
        match freq.0 {
            0 ..= 99 => {
                unsafe { syscon.raw.fmccr.modify(|_, w| w.flashtim().bits((freq.0/11) as u8 - 1)) };
            }
            100 ..= 115 => {
                unsafe { syscon.raw.fmccr.modify(|_, w| w.flashtim().bits( 9 )) };
            }
            116 ..= 130=> {
                unsafe { syscon.raw.fmccr.modify(|_, w| w.flashtim().bits( 10 )) };
            }
            _ => {
                unsafe { syscon.raw.fmccr.modify(|_, w| w.flashtim().bits( 11 )) };
            }
        }
    }

    /// Requirements solver - tries to generate and configure a clock configuration
    /// from (partial) requirements.
    ///
    /// Can be called only once, to not invalidate previous configurations
    pub fn configure(self, anactrl: &mut Anactrl, pmc: &mut Pmc, syscon: &mut Syscon) -> Result<Clocks> {
        if unsafe { CONFIGURED } {
            return Err(ClocksError::AlreadyConfigured);
        }

        let freq: Megahertz = self.system_frequency.unwrap_or(DEFAULT_FREQ);

        // turn on FRO192M: clear bit 5, according to `fsl_power.h` from the SDK
        // unsafe { pmc.raw.pdruncfgclr0.write(|w| w.bits(1u32 << 5)) };
        // but it's hidden in UM, so let's assume this is always cleared

        // turn on 1mhz, 12mhz and 96mhz clocks
        anactrl.raw.fro192m_ctrl.modify(|_, w| w.ena_96mhzclk().enable());
        anactrl.raw.fro192m_ctrl.modify(|_, w| w.ena_12mhzclk().enable());

        syscon.raw.clock_ctrl.modify(|_, w| w
            .fro1mhz_clk_ena().enable()
            .fro1mhz_utick_ena().enable()
        );

        let (main_clock, sys_divider) = Self::get_clock_source_and_div_for_freq(freq, pmc, syscon);
        Self::set_new_clock_source(freq, main_clock, sys_divider, syscon);

        unsafe { CONFIGURED = true };

        Ok(Clocks {
            main_clock,
            system_frequency: Hertz::try_from(freq).unwrap(),
        })
    }

    /// Same as above, but allows clock to be changed after an initial configuration.
    /// This is unsafe because it's up to the developer to ensure the new configuration is okay for
    /// the device peripherals being used.
    pub unsafe fn reconfigure(self, _clocks: Clocks, pmc: &mut Pmc, syscon: &mut Syscon) -> Clocks {
        let freq: Megahertz = self.system_frequency.unwrap_or(DEFAULT_FREQ);

        let (main_clock, sys_divider) = Self::get_clock_source_and_div_for_freq(freq, pmc, syscon);

        Self::set_new_clock_source(freq, main_clock, sys_divider, syscon);

        Clocks {
            main_clock,
            system_frequency: Hertz::try_from(freq).unwrap(),
        }
    }
}
