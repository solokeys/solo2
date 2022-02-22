#![no_main]
#![no_std]

extern crate panic_semihosting;
// extern crate panic_halt;
use cortex_m_rt::entry;

#[allow(unused_imports)]
use hal::prelude::*;
#[allow(unused_imports)]
use lpc55_hal as hal;

use usb_device::test_class::TestClass;
use usb_device::device::{UsbDeviceBuilder,UsbVidPid};
use hal::drivers::{
    pins,
    UsbBus,
    Timer,
};

#[entry]
fn main() -> ! {

    let hal = hal::new();

    let mut anactrl = hal.anactrl;
    let mut syscon = hal.syscon;
    let mut pmc = hal.pmc;

    let mut iocon = hal.iocon.enabled(&mut syscon);
    let usb0_vbus_pin = pins::Pio0_22::take().unwrap().into_usb0_vbus_pin(&mut iocon);
    iocon.disabled(&mut syscon); // perfectionist ;)

    let clocks = hal::ClockRequirements::default()
        .system_frequency(96.MHz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .expect("Clock configuration failed");


    let mut _delay_timer = Timer::new(hal.ctimer.0.enabled(&mut syscon, clocks.support_1mhz_fro_token().unwrap()));

    // Can use compile to use either the "HighSpeed" or "FullSpeed" USB peripheral.
    // Default is full speed.
    #[cfg(feature = "highspeed-usb-example")]
    let usb_peripheral = hal.usbhs.enabled_as_device(
        &mut anactrl,
        &mut pmc,
        &mut syscon,
        &mut _delay_timer,
        clocks.support_usbhs_token()
                        .unwrap()
    );

    #[cfg(not(feature = "highspeed-usb-example"))]
    let usb_peripheral = hal.usbfs.enabled_as_device(
        &mut anactrl,
        &mut pmc,
        &mut syscon,
        clocks.support_usbfs_token()
                        .unwrap()
    );


    let usb_bus = UsbBus::new(usb_peripheral, usb0_vbus_pin);

    const VID: u16 = 0x16c0;
    const PID: u16 = 0x05dc;
    const MANUFACTURER: &'static str = "TestClass Manufacturer";
    const PRODUCT: &'static str = "virkkunen.net usb-device TestClass";
    const SERIAL_NUMBER: &'static str = "TestClass Serial";

    let mut test = TestClass::new(&usb_bus);
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(VID, PID))
        .manufacturer(MANUFACTURER)
        .product(PRODUCT)
        .serial_number(SERIAL_NUMBER)
        .max_packet_size_0(64)
        .build();


    loop {
        if usb_dev.poll(&mut [&mut test]) {
            test.poll();
        }
    }
}
