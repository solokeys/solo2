use core::ops::Deref;
use embedded_time::duration::Extensions;

use crate::raw;
use crate::traits::wg::timer::CountDown;
use crate::drivers::timer;
use crate::peripherals::{
    anactrl,
    pmc,
    syscon,
    ctimer,
};
use crate::typestates::{
    init_state,
    usbhs_mode,
    // ValidUsbClockToken,
    // Fro96MHzEnabledToken,
    ClocksSupportUsbhsToken,
};

use crate::traits::usb::{
    Usb,
    UsbSpeed,
};

// Main struct
pub struct Usbhs<State: init_state::InitState = init_state::Unknown, Mode: usbhs_mode::UsbhsMode = usbhs_mode::Unknown> {
    pub(crate) raw_phy: raw::USBPHY,
    pub(crate) raw_hsd: raw::USB1,
    pub(crate) raw_hsh: raw::USBHSH,
    _state: State,
    _mode: Mode,
}

pub type EnabledUsbhsDevice = Usbhs<init_state::Enabled, usbhs_mode::Device>;
pub type EnabledUsbhsHost = Usbhs<init_state::Enabled, usbhs_mode::Host>;

impl Deref for EnabledUsbhsDevice {
    type Target = raw::usb1::RegisterBlock;
    fn deref(&self) -> &Self::Target {
        &self.raw_hsd
    }
}

unsafe impl Sync for EnabledUsbhsDevice {}

impl Usb<init_state::Enabled> for EnabledUsbhsDevice {
    const SPEED: UsbSpeed = UsbSpeed::HighSpeed;
    // const NUM_ENDPOINTS: usize = 1 + 5;
}

impl Usbhs {
    pub fn new(raw_phy: raw::USBPHY, raw_hsd: raw::USB1, raw_hsh: raw::USBHSH) -> Self {
        Usbhs {
            raw_phy,
            raw_hsd,
            raw_hsh,
            _state: init_state::Unknown,
            _mode: usbhs_mode::Unknown,
        }
    }
}

impl<State: init_state::InitState, Mode: usbhs_mode::UsbhsMode> Usbhs<State, Mode> {
    pub fn release(self) -> (raw::USB1, raw::USBHSH) {
        (self.raw_hsd, self.raw_hsh)
    }


    pub fn enabled_as_device(
        mut self,
        anactrl: &mut anactrl::Anactrl,
        pmc: &mut pmc::Pmc,
        syscon: &mut syscon::Syscon,
        timer: &mut timer::Timer<impl ctimer::Ctimer<init_state::Enabled>>,
        // lock_fro_to_sof: bool, // we always lock to SOF
        _clocks_token: ClocksSupportUsbhsToken,
    ) -> EnabledUsbhsDevice {


        // Reset devices
        syscon.reset(&mut self.raw_hsh);
        syscon.reset(&mut self.raw_hsd);
        syscon.reset(&mut self.raw_phy);

        // Briefly turn on host controller to enable device control of USB1 port
        syscon.enable_clock(&mut self.raw_hsh);

        self.raw_hsh.portmode.modify(|_,w| {
            w.dev_enable().set_bit()
        });

        syscon.disable_clock(&mut self.raw_hsh);

        // Power on 32M crystal for HS PHY and connect to USB PLL
        pmc.raw.pdruncfg0.modify(|_,w| w.pden_xtal32m().poweredon());
        pmc.raw.pdruncfg0.modify(|_,w| w.pden_ldoxo32m().poweredon());
        anactrl.raw.xo32m_ctrl.modify(|_,w| w.enable_pll_usb_out().set_bit() );

        pmc.power_on(&mut self.raw_phy);

        // Give long delay for PHY to be ready
        timer.start(5000_u32.microseconds());
        nb::block!(timer.wait()).ok();

        syscon.enable_clock(&mut self.raw_phy);

        // Initial config of PHY control registers
        self.raw_phy.ctrl.write(|w| {
            w
            .sftrst().clear_bit()
        });

        self.raw_phy.pll_sic.modify(|_,w|
            w
            .pll_div_sel().bits(6) /* 16MHz = xtal32m */
            .pll_reg_enable().set_bit()
        );

        self.raw_phy.pll_sic_clr.write(|w| unsafe {
            // must be done, according to SDK.
            w.bits(1 << 16 /* mystery bit */)
        });

        // Must wait at least 15 us for pll-reg to stabilize
        timer.start(15.microseconds());
        nb::block!(timer.wait()).ok();

        self.raw_phy.pll_sic.modify(|_,w| {
            w
            .pll_power().set_bit()
            .pll_en_usb_clks().set_bit()
        });

        self.raw_phy.ctrl.modify(|_,w| {
            w
            .clkgate().clear_bit()
            .enautoclr_clkgate().set_bit()
            .enautoclr_phy_pwd().clear_bit()
        });

        // Turn on everything in PHY
        self.raw_phy.pwd.write(|w| unsafe { w.bits(0) } );

        // turn on USB1 device controller access
        syscon.enable_clock(&mut self.raw_hsd);

        Usbhs {
            raw_phy: self.raw_phy,
            raw_hsd: self.raw_hsd,
            raw_hsh: self.raw_hsh,
            _state: init_state::Enabled(()),
            _mode: usbhs_mode::Device,
        }
    }

    pub fn borrow<F: Fn(&mut Self) -> () >(&mut self, func: F) {
        func(self);
    }

}

#[derive(Debug)]
pub struct UsbHsDevInfo {
    maj_rev: u8,
    min_rev: u8,
    err_code: u8,
    frame_nr: u16,
}

impl EnabledUsbhsDevice {
    pub fn info(&self) -> UsbHsDevInfo {
        // technically, e.g. maj/min rev need only the clock, and not the power enabled
        UsbHsDevInfo {
            maj_rev: self.raw_hsd.info.read().majrev().bits(),
            min_rev: self.raw_hsd.info.read().minrev().bits(),
            err_code: self.raw_hsd.info.read().err_code().bits(),
            frame_nr: self.raw_hsd.info.read().frame_nr().bits(),
        }
    }

    pub fn disable_high_speed(&mut self) {
        // Note: Application Note https://www.nxp.com/docs/en/application-note/TN00071.zip
        // states that devcmdstat.force_fs (bit 21) might also be used.
        self.raw_phy.pwd_set.write(|w| unsafe { w.bits(1<<12) /* TXPWDV2I */} );
    }
}

impl<State: init_state::InitState> Usbhs<State, usbhs_mode::Device> {
    /// Disables the USB HS peripheral, assumed in device mode
    pub fn disabled(
        mut self,
        pmc: &mut pmc::Pmc,
        syscon: &mut syscon::Syscon,
    ) -> Usbhs<init_state::Disabled, usbhs_mode::Device> {

        syscon.disable_clock(&mut self.raw_hsd);

        syscon.disable_clock(&mut self.raw_phy);

        pmc.power_off(&mut self.raw_phy);

        pmc.raw.pdruncfg0.modify(|_,w| w.pden_xtal32m().poweredoff());
        pmc.raw.pdruncfg0.modify(|_,w| w.pden_ldoxo32m().poweredoff());

        Usbhs {
            raw_phy: self.raw_phy,
            raw_hsd: self.raw_hsd,
            raw_hsh: self.raw_hsh,
            _state: init_state::Disabled,
            _mode: usbhs_mode::Device,
        }
    }
}

impl From<(raw::USBPHY, raw::USB1, raw::USBHSH)> for Usbhs {
    fn from(raw: (raw::USBPHY, raw::USB1, raw::USBHSH)) -> Self {
        Usbhs::new(raw.0, raw.1, raw.2)
    }
}
