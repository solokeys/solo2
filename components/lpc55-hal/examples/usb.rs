#![no_main]
#![no_std]

extern crate panic_semihosting;
// extern crate panic_halt;
use cortex_m_rt::entry;
// use cortex_m_semihosting::{dbg, heprintln};

#[allow(unused_imports)]
use hal::prelude::*;
#[allow(unused_imports)]
use lpc55_hal as hal;

use usbd_serial::{CdcAcmClass, /*SerialPort*/};
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
use hal::drivers::{
    pins,
    UsbBus,
    Timer,
};

#[entry]
fn main() -> ! {

    let hal = hal::new();

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;

    let mut gpio = hal.gpio.enabled(&mut syscon);
    let mut iocon = hal.iocon.enabled(&mut syscon);

    let mut red_led = pins::Pio1_6::take().unwrap()
        .into_gpio_pin(&mut iocon, &mut gpio)
        .into_output(hal::drivers::pins::Level::High); // start turned off

    let usb0_vbus_pin = pins::Pio0_22::take().unwrap()
        .into_usb0_vbus_pin(&mut iocon);

    iocon.disabled(&mut syscon).release(); // save the environment :)


    let clocks = hal::ClockRequirements::default()
        // .system_frequency(24.mhz())
        // .system_frequency(72.mhz())
        .system_frequency(96.MHz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .unwrap();

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

    let mut cdc_acm = CdcAcmClass::new(&usb_bus, 8);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x1209, 0xcc1d))
        .manufacturer("nickray")
        .product("Demo Demo Demo")
        .serial_number("2019-10-10")
        .device_release(0x0123)
        // Must be 64 bytes for HighSpeed
        .max_packet_size_0(64)
        // .device_class(USB_CLASS_CDC)
        .build();

    // dbg!("main loop");
    let mut need_zlp = false;
    let mut buf = [0u8; 8];
    let mut size = 0;
    let mut buf_in_use = false;
    loop {
        // if !usb_dev.poll(&mut []) {
        // if !usb_dev.poll(&mut [&mut serial]) {
        if !usb_dev.poll(&mut [&mut cdc_acm]) {
            continue;
        }

        if !(buf_in_use || need_zlp) {
            match cdc_acm.read_packet(&mut buf) {
                Ok(count) => {
                    size = count;
                    buf_in_use = true;
                    // dbg!(&buf[..count]);
                    // if count > 1 {
                    //     dbg!(count);
                    // }
                },
                _ => {}
            }
        }

        if buf_in_use {
            red_led.set_low().ok(); // Turn on
            match cdc_acm.write_packet(&buf[..size]) {
                Ok(count) => {
                    assert!(count == size);
                    buf_in_use = false;
                    need_zlp = size == 8;
                },
                _ => {}
            }
            red_led.set_high().ok(); // Turn off
        }

        if need_zlp {
            match cdc_acm.write_packet(&[]) {
                Ok(count) => {
                    assert!(count == 0);
                    need_zlp = false;
                },
                _ => {}
            }
        }


        // let mut buf = [0u8; 512];

        // match serial.read(&mut buf) {
        //     Ok(count) if count > 0 => {
        //         assert!(count == 1);
        //         // hprintln!("received some data on the serial port: {:?}", &buf[..count]).ok();
        //         // cortex_m_semihosting::hprintln!("received:\n{}", core::str::from_utf8(&buf[..count]).unwrap()).ok();
        //         red_led.set_low().ok(); // Turn on

        //         // cortex_m_semihosting::hprintln!("read {:?}", &buf[..count]).ok();
        //         cortex_m_semihosting::hprintln!("read {:?}", count).ok();

        //         // Echo back in upper case
        //         for c in buf[0..count].iter_mut() {
        //             if (0x61 <= *c && *c <= 0x7a) || (0x41 <= *c && *c <= 0x5a) {
        //                 *c ^= 0x20;
        //             }
        //         }

        //         let mut write_offset = 0;
        //         while write_offset < count {
        //             match serial.write(&buf[write_offset..count]) {
        //                 Ok(len) if len > 0 => {
        //                     write_offset += len;
        //                     cortex_m_semihosting::hprintln!("wrote {:?}", len).ok();

        //                 },
        //                 _ => {},
        //             }
        //         }

        //         // hprintln!("wrote it back").ok();
        //     }
        //     _ => {}
        // }

        // red_led.set_high().ok(); // Turn off
    }

}
