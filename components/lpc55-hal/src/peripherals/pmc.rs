//! API for power management (PMC) - always on
//!
//! The PMC peripheral is described in the user manual, chapter 13.
//!
//! We are mostly concerned with power to analog peripherals.
//!

crate::wrap_always_on_peripheral!(Pmc, PMC);

// The UM does not list everything.
// This is what `fsl_power.h` from the SDK reveals:
//
// kPDRUNCFG_PD_DCDC         = (1UL << 0),
// kPDRUNCFG_PD_BIAS         = (1UL << 1),
// kPDRUNCFG_PD_BODCORE      = (1UL << 2),
// kPDRUNCFG_PD_BODVBAT      = (1UL << 3),
// kPDRUNCFG_PD_FRO1M        = (1UL << 4),
// kPDRUNCFG_PD_FRO192M      = (1UL << 5),
// kPDRUNCFG_PD_FRO32K       = (1UL << 6),
// kPDRUNCFG_PD_XTAL32K      = (1UL << 7),
// kPDRUNCFG_PD_XTAL32M      = (1UL << 8),
// kPDRUNCFG_PD_PLL0         = (1UL << 9),
// kPDRUNCFG_PD_PLL1         = (1UL << 10),
// kPDRUNCFG_PD_USB0_PHY     = (1UL << 11),
// kPDRUNCFG_PD_USB1_PHY     = (1UL << 12),
// kPDRUNCFG_PD_COMP         = (1UL << 13),
// kPDRUNCFG_PD_TEMPSENS     = (1UL << 14),
// kPDRUNCFG_PD_GPADC        = (1UL << 15),
// kPDRUNCFG_PD_LDOMEM       = (1UL << 16),
// kPDRUNCFG_PD_LDODEEPSLEEP = (1UL << 17),
// kPDRUNCFG_PD_LDOUSBHS     = (1UL << 18),
// kPDRUNCFG_PD_LDOGPADC     = (1UL << 19),
// kPDRUNCFG_PD_LDOXO32M     = (1UL << 20),
// kPDRUNCFG_PD_LDOFLASHNV   = (1UL << 21),
// kPDRUNCFG_PD_RNG          = (1UL << 22),
// kPDRUNCFG_PD_PLL0_SSCG    = (1UL << 23),
// kPDRUNCFG_PD_ROM          = (1UL << 24),

impl Pmc {
    /// Enables the power for a peripheral or other hardware component
    pub fn power_on<P: PowerControl>(&mut self, peripheral: &mut P) {
        peripheral.powered_on(self);
    }

    /// Disable the power
    pub fn power_off<P: PowerControl>(&mut self, peripheral: &mut P) {
        peripheral.powered_off(self);
    }

    /// Check if peripheral is powered
    pub fn is_powered<P: PowerControl>(&self, peripheral: &P) -> bool {
        peripheral.is_powered(&self)
    }
}

pub trait PowerControl {
    /// Internal method
    fn powered_on(&self, pmc: &mut Pmc);

    /// Internal method
    fn powered_off(&self, pmc: &mut Pmc);

    /// Internal method
    fn is_powered(&self, pmc: &Pmc) -> bool;
}

// impl ClockControl for raw::UTICK {
//     fn enable_clock<'h>(&self, h: &'h mut Handle) -> &'h mut Handle {
//         h.ahbclkctrl1.modify(|_, w| w.utick0().enable());
//         h
//     }

//     fn disable_clock<'h>(&self, h: &'h mut Handle) -> &'h mut Handle {
//         h.ahbclkctrl1.modify(|_, w| w.utick0().disable());
//         h
//     }

//     fn is_clock_enabled(&self, h: &Handle) -> bool {
//         h.ahbclkctrl1.read().utick0().is_enable()
//     }
// }

// impl PowerControl for raw::USB0 {
//     fn powered_on(&self, pmc: &mut Pmc) {
//         // Enable the power to the USB0 PHY by clearing the bit PDEN_USBFSPHY in the PDRUNCFG0 register
//         pmc.raw
//             .pdruncfg0
//             .modify(|_, w| w.pden_usbfsphy().poweredon());
//     }

//     /// Internal method
//     fn powered_off(&self, pmc: &mut Pmc) {
//         pmc.raw
//             .pdruncfg0
//             .modify(|_, w| w.pden_usbfsphy().poweredoff());
//     }

//     /// Internal method
//     fn is_powered(&self, pmc: &Pmc) -> bool {
//         pmc.raw.pdruncfg0.read().pden_usbfsphy().is_poweredon()
//     }
// }

// TODO: use the clr/set registers
macro_rules! impl_power_control {
    ($power_control:ty, $register:ident) => {
        impl PowerControl for $power_control {
            fn powered_on(&self, pmc: &mut Pmc) {
                // pmc.raw.pdruncfg0clr.write(|w| w.bits(1u32 << <proper offset>));
                pmc.raw.pdruncfg0.modify(|_, w| w.$register().poweredon());
            }

            fn powered_off(&self, pmc: &mut Pmc) {
                pmc.raw.pdruncfg0.modify(|_, w| w.$register().poweredoff());
            }

            fn is_powered(&self, pmc: &Pmc) -> bool {
                pmc.raw.pdruncfg0.read().$register().is_poweredon()
            }
        }
    };

    ($power_control:ty, $register1:ident, $register2:ident) => {
        impl PowerControl for $power_control {
            fn powered_on(&self, pmc: &mut Pmc) {
                pmc.raw.pdruncfg0.modify(|_, w| w.$register1().poweredon());
                pmc.raw.pdruncfg0.modify(|_, w| w.$register2().poweredon());
            }

            fn powered_off(&self, pmc: &mut Pmc) {
                pmc.raw.pdruncfg0.modify(|_, w| w.$register1().poweredoff());
                pmc.raw.pdruncfg0.modify(|_, w| w.$register2().poweredoff());
            }

            fn is_powered(&self, pmc: &Pmc) -> bool {
                pmc.raw.pdruncfg0.read().$register1().is_poweredon() &&
                pmc.raw.pdruncfg0.read().$register2().is_poweredon()
            }
        }
    };
}

// well maybe there needs to be a USBFS peripheral with power control,
// and on top of that USBFSD, USBFSHM, USBFSHS... to make this all logical.
impl_power_control!(raw::USB0, pden_usbfsphy);
impl_power_control!(raw::USBPHY, pden_usbhsphy, pden_ldousbhs);
impl_power_control!(raw::ADC0, pden_auxbias);
impl_power_control!(crate::typestates::ClocksSupport32KhzFroToken, pden_fro32k);
