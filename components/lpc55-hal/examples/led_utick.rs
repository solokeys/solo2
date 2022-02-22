#![no_main]
#![no_std]

// extern crate panic_semihosting;  // 4004 bytes
extern crate panic_halt; // 672 bytes

use cortex_m_rt::entry;

use lpc55_hal as hal;
use hal::{
    drivers::pins::Level,
    prelude::*,
};

macro_rules! kitt {
    ($utick:ident, $($led:ident),+ ) => ($(

        // low = on
        $led.set_low().unwrap();

        $utick.start(1_000_000u32);
        $utick.blocking_wait();

        // high = off
        $led.set_high().unwrap();

    )*);
}

#[entry]
fn main() -> ! {
    let hal = hal::new();

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;

    let clocks = hal::ClockRequirements::default()
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .unwrap();

    let token = clocks.support_utick_token().unwrap();
    let mut utick = hal.utick.enabled(&mut syscon, &token);

    let mut gpio = hal.gpio.enabled(&mut syscon);
    let mut iocon = hal.iocon.enabled(&mut syscon);
    let pins = hal::Pins::take().unwrap();

    // R = pio1_6
    // G = pio1_7
    // B = pio1_4

    let mut red = pins
        .pio1_6
        .into_gpio_pin(&mut iocon, &mut gpio)
        .into_output(Level::High);

    let mut green = pins
        .pio1_7
        .into_gpio_pin(&mut iocon, &mut gpio)
        .into_output(Level::High);

    let mut blue = pins
        .pio1_4
        .into_gpio_pin(&mut iocon, &mut gpio)
        .into_output(Level::High);

    loop {
        kitt!(utick, red, green, blue);
    }
}
