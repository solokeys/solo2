#![no_main]
#![no_std]
/// Simple demo of the real time clock peripheral.

extern crate panic_semihosting;  // 4004 bytes
// extern crate panic_halt; // 672 bytes

use cortex_m_semihosting::{heprintln};
use cortex_m_rt::entry;

use lpc55_hal as hal;
use hal::{
    prelude::*,
};

pub fn delay_cycles(delay: u64) {
    let current = hal::get_cycle_count() as u64;
    let mut target = current + delay;
    if target > 0xFFFF_FFFF {
        // wait for wraparound
        target -= 0xFFFF_FFFF;
        while target < hal::get_cycle_count() as u64 { continue; }
    }
    while target > hal::get_cycle_count() as u64 { continue; }
}

#[entry]
fn main() -> ! {
    let hal = hal::new();

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;

    let clocks = hal::ClockRequirements::default()
        .system_frequency(96.MHz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .unwrap();

    let token_32k_fro = clocks.enable_32k_fro(&mut pmc);

    let mut rtc = hal.rtc.enabled(&mut syscon, token_32k_fro);

    hal::enable_cycle_counter();

    // RTC is not reset by system reset, only by direct call or by power reboot.
    rtc.reset();

    loop {
        delay_cycles(10_000_000);
        heprintln!("{:?}", rtc.uptime()).ok();
    }
}
