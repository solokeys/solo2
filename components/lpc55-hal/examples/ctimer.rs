#![no_main]
#![no_std]

extern crate panic_semihosting;  // 4004 bytes
// extern crate panic_halt; // 672 bytes

use cortex_m_rt::entry;
use cortex_m_semihosting::dbg;
use cortex_m_semihosting::heprintln;

use lpc55_hal as hal;
use hal::prelude::*;

use hal::drivers::timer::Elapsed;

#[macro_use(block)]
extern crate nb;

use hal::drivers::{
    Timer,
};

#[entry]
fn main() -> ! {

    heprintln!("Hello ctimer").unwrap();

    // Get pointer to all device peripherals.
    let mut hal = hal::new();

    let clocks = hal::ClockRequirements::default()
        .system_frequency(12.MHz())
        .configure(&mut hal.anactrl, &mut hal.pmc, &mut hal.syscon)
        .unwrap();

    let ctimer = hal.ctimer.1.enabled(&mut hal.syscon, clocks.support_1mhz_fro_token().unwrap());
    let mut cdriver = Timer::new(ctimer);

    heprintln!("looping 1 Hz").unwrap();
    let mut c = 0;
    loop {
        cdriver.start(1_000_000.microseconds());
        dbg!(c * 1_000_000);
        dbg!(cdriver.elapsed().0);
        c += 1;
        block!(cdriver.wait()).unwrap(); // blocks for 1 second
    }
}
