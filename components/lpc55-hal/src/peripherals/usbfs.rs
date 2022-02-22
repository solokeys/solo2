use core::ops::Deref;
use crate::raw;
use crate::peripherals::{
    anactrl,
    pmc,
    syscon,
};
use crate::typestates::{
    init_state,
    usbfs_mode,
    // ValidUsbClockToken,
    // Fro96MHzEnabledToken,
    ClocksSupportUsbfsToken,
};

use crate::traits::usb::{
    Usb,
    UsbSpeed,
};



// Main struct
pub struct Usbfs<State: init_state::InitState = init_state::Unknown, Mode: usbfs_mode::UsbfsMode = usbfs_mode::Unknown> {
    pub(crate) raw_fsd: raw::USB0,
    pub(crate) raw_fsh: raw::USBFSH,
    _state: State,
    _mode: Mode,
}

pub type EnabledUsbfsDevice = Usbfs<init_state::Enabled, usbfs_mode::Device>;
pub type EnabledUsbfsHost = Usbfs<init_state::Enabled, usbfs_mode::Host>;

impl Deref for EnabledUsbfsDevice {
    type Target = raw::usb1::RegisterBlock;
    fn deref(&self) -> &Self::Target {
        &self.raw_fsd
    }
}

unsafe impl Sync for EnabledUsbfsDevice {}

impl Usb<init_state::Enabled> for EnabledUsbfsDevice {
    const SPEED: UsbSpeed = UsbSpeed::FullSpeed;
    // const NUM_ENDPOINTS: usize = 1 + 5;
}

impl Usbfs {
    pub fn new(raw_fsd: raw::USB0, raw_fsh: raw::USBFSH) -> Self {
        Usbfs {
            raw_fsd,
            raw_fsh,
            _state: init_state::Unknown,
            _mode: usbfs_mode::Unknown,
        }
    }
}

impl<State: init_state::InitState, Mode: usbfs_mode::UsbfsMode> Usbfs<State, Mode> {
    pub fn release(self) -> (raw::USB0, raw::USBFSH) {
        (self.raw_fsd, self.raw_fsh)
    }

    pub fn enabled_as_device(
        mut self,
        anactrl: &mut anactrl::Anactrl,
        pmc: &mut pmc::Pmc,
        syscon: &mut syscon::Syscon,
        // lock_fro_to_sof: bool, // we always lock to SOF
        _clocks_token: ClocksSupportUsbfsToken,
    ) -> EnabledUsbfsDevice {

        // Configure clock input: Fro96MHz divided by 2 = 48MHz
        // TODO: disable this again in `self.disable`.
        unsafe { syscon.raw.usb0clkdiv.modify(|_, w| w.div().bits(1)) };
        syscon.raw.usb0clkdiv.modify(|_, w| w.halt().run());
        syscon.raw.usb0clksel.modify(|_, w| w.sel().enum_0x3()); // Fro96MHz
        while syscon.raw.usb0clkdiv.read().reqflag().is_ongoing() {}

        // turn on USB0 PHY
        pmc.power_on(&mut self.raw_fsd);

        // reset and turn on clock
        syscon.reset(&mut self.raw_fsd);
        syscon.enable_clock(&mut self.raw_fsd);

        // Switch USB0 to "device" mode (default is "host")
        syscon.enable_clock(&mut self.raw_fsh);
        // NB!!! This will crash your debugger soo bad if usb0clk is not setup !!!
        self.raw_fsh.portmode.modify(|_, w| w.dev_enable().set_bit());
        syscon.disable_clock(&mut self.raw_fsh);

        // Turn on USB1 SRAM
        // TODO: Maybe ask to pass in an enabled USB1 SRAM?
        // Otherwise, do we turn this off in `disabled`?
        // reg_modify!(crate, SYSCON, ahbclkctrl2, usb1_ram, enable);
        syscon.raw.ahbclkctrl2.modify(|_, w| w.usb1_ram().enable());

        // Lock Fro192MHz to USB SOF packets
        // if lock_fro_to_sof {
            anactrl.raw.fro192m_ctrl.modify(|_, w| w.usbclkadj().set_bit());
            while anactrl.raw.fro192m_ctrl.read().usbmodchg().bit_is_set() {}
        // }


        Usbfs {
            raw_fsd: self.raw_fsd,
            raw_fsh: self.raw_fsh,
            _state: init_state::Enabled(()),
            _mode: usbfs_mode::Device,
        }
    }

}

#[derive(Debug)]
pub struct UsbFsDevInfo {
    maj_rev: u8,
    min_rev: u8,
    err_code: u8,
    frame_nr: u16,
}

impl EnabledUsbfsDevice {
    pub fn info(&self) -> UsbFsDevInfo {
        // technically, e.g. maj/min rev need only the clock, and not the power enabled
        UsbFsDevInfo {
            maj_rev: self.raw_fsd.info.read().majrev().bits(),
            min_rev: self.raw_fsd.info.read().minrev().bits(),
            err_code: self.raw_fsd.info.read().err_code().bits(),
            frame_nr: self.raw_fsd.info.read().frame_nr().bits(),
        }
    }
}

impl<State: init_state::InitState> Usbfs<State, usbfs_mode::Device> {
    /// Disables the USB FS peripheral, assumed in device mode
    pub fn disabled(
        mut self,
        pmc: &mut pmc::Pmc,
        syscon: &mut syscon::Syscon,
    ) -> Usbfs<init_state::Disabled, usbfs_mode::Device> {
        pmc.power_off(&mut self.raw_fsd);
        syscon.disable_clock(&mut self.raw_fsd);

        Usbfs {
            raw_fsd: self.raw_fsd,
            raw_fsh: self.raw_fsh,
            _state: init_state::Disabled,
            _mode: usbfs_mode::Device,
        }
    }
}

impl From<(raw::USB0, raw::USBFSH)> for Usbfs {
    fn from(raw: (raw::USB0, raw::USBFSH)) -> Self {
        Usbfs::new(raw.0, raw.1)
    }
}
