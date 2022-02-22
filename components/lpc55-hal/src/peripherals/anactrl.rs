//! API for analog control (ANACTRL) - always on

// use core::marker::PhantomData;
// use crate::typestates::Fro96MHzEnabledToken;
// use crate::{ raw,};

// UM says:
// NOTE: The clock to analog controller module is enabled during boot time
// by the boot loader and it should always stay enabled
//
// So for safety, should enforce it's enabled
crate::wrap_always_on_peripheral!(Anactrl, ANACTRL);

impl Anactrl {
    // pub(crate) fn latch_fro_to_usbfs_sof(&mut self) {
    //     // "If this bit is set and the USB peripheral is enabled into full-speed
    //     // device mode, the USB block will provide FRO clock adjustments to
    //     // lock it to the host clock using the SOF packets."
    //     self.raw.fro192m_ctrl.modify(|_, w| w.usbclkadj().set_bit());
    //     // TODO: do we need any stabilisation? e.g.
    //     // "USBMODCHG If it reads as 1 when reading the DAC_TRIM field and
    //     // USBCLKADJ=1, it should be re-read until it is 0."
    //     // <-- this is mentioned in the UM, but missing in the PAC.
    // }

    pub fn is_12mhzclk_enabled(&self) -> bool {
        self.raw.fro192m_ctrl.read().ena_12mhzclk().is_enable()
    }

    /// Supposedly, this is needed for Flash, hence enabled by default
    pub fn is_48mhzclk_enabled(&self) -> bool {
        self.raw.fro192m_ctrl.read().ena_48mhzclk().is_enable()
    }

    /// UM says not enabled by default, empiricially seems it is
    pub fn is_96mhzclk_enabled(&self) -> bool {
        // ewww this hard-faults if 12MHz clock is main clock?!
        self.raw.fro192m_ctrl.read().ena_96mhzclk().is_enable() &&
        self.raw.fro192m_status.read().clk_valid().is_clkout()
    }

    // pub(crate) fn enable_96mhzclk(&self) {
    //     self.raw.fro192m_ctrl.modify(|_, w| w.ena_96mhzclk().enable());
    // }

    // pub fn fro96mhz_enabled_token(&self) -> Option<Fro96MHzEnabledToken> {
    //     if self.is_96mhzclk_enabled() {
    //        Some(Fro96MHzEnabledToken{__: PhantomData})
    //     } else {
    //         None
    //     }
    // }
}
