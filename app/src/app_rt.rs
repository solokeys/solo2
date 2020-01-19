//! main app in cortex-m-rt version
//!
//! While we intend to use RTFM (see `main_rtfm.rs`.),
//! we try to keep this RT-version on feature parity.

#![no_std]
#![no_main]

use app::hal;

use usbd_ctaphid::CtapHid;
use usbd_ctaphid::insecure::InsecureRamAuthenticator;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
use hal::drivers::{
    pins,
    UsbBus,
};

// bring traits in scope
use hal::prelude::*;

#[app::entry]
fn main() -> ! {

    let hal = hal::new();

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;

    let mut gpio = hal.gpio.enabled(&mut syscon);
    let mut iocon = hal.iocon.enabled(&mut syscon);

    let mut _red_led = pins::Pio1_6::take().unwrap()
        .into_gpio_pin(&mut iocon, &mut gpio)
        .into_output(hal::drivers::pins::Level::High); // start turned off

    let usb0_vbus_pin = pins::Pio0_22::take().unwrap()
        .into_usb0_vbus_pin(&mut iocon);

    iocon.disabled(&mut syscon).release(); // save the environment :)


    let clocks = hal::ClockRequirements::default()
        .support_usbfs()
        .system_frequency(96.mhz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .expect("Clock configuration failed");

    let token = clocks.support_usbfs_token().unwrap();

    let usbfsd = hal.usbfs.enabled_as_device(
        &mut anactrl,
        &mut pmc,
        &mut syscon,
        token,
    );

    let usb_bus = UsbBus::new(usbfsd, usb0_vbus_pin);

    let mut authenticator = InsecureRamAuthenticator::default();

    let mut ctap_hid = CtapHid::new(&usb_bus, &mut authenticator);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x1209, 0xF1D0))
        .manufacturer("SoloKeys")
        .product("🐝")
        .serial_number("20/20")
        .device_release(0x0123)
        .build();

    loop {
        if !usb_dev.poll(&mut [&mut ctap_hid]) {
            continue;
        }
    }
}
