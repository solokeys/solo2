#![no_main]
#![no_std]

extern crate panic_semihosting;
use cortex_m::iprintln;
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;

use hal::prelude::*;
use lpc55_hal as hal;

#[entry]
fn main() -> ! {
    let hal = hal::new();
    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;
    let mut gpio = hal.gpio.enabled(&mut syscon);
    let mut iocon = hal.iocon.enabled(&mut syscon);

    let mut cp = unsafe { hal::raw::CorePeripherals::steal() };
    let dp = unsafe { hal::raw::Peripherals::steal() };

    hprintln!("traceclksel = {:x?}", dp.SYSCON.traceclksel.read().bits()).ok();
    hprintln!("traceclkdiv = {:x?}", dp.SYSCON.traceclkdiv.read().bits()).ok();
    hprintln!("traceclkdiv.div = {:x?}", dp.SYSCON.traceclkdiv.read().div().bits()).ok();
    hprintln!("traceclkdiv.halt = {:x?}", dp.SYSCON.traceclkdiv.read().halt().bits()).ok();
    // unsafe { dp.SYSCON.traceclksel.write(|w| w.sel().bits(0)); }
    // unsafe { dp.SYSCON.traceclkdiv.write(|w| w.div().bits(1)); }

    // iocon.set_pio_0_8_swo_func();
    iocon.set_pio_0_10_swo_func();
    // hprintln!("pio_0_8 = {:?}", iocon.get_pio_0_8_func());
    // hprintln!("pio_0_10 = {:?}", iocon.get_pio_0_10_func());
    // hprintln!("traceclkdiv = {:?}", dp.SYSCON.traceclkdiv.read().bits());

    // let mut cp = unsafe { hal::raw::CorePeripherals::steal() };
    let stim = &mut cp.ITM.stim[0];

    // dbg!(unsafe { &(*hal::raw::TPIU::ptr()) }.sppr.read() );
    // unsafe {      &(*hal::raw::TPIU::ptr())  .sppr.write(2) };
    // dbg!(unsafe { &(*hal::raw::TPIU::ptr()) }.sppr.read() );

    // UM kind of says it's not enabled, but it actually is
    // let iocon = iocon.enabled(&mut syscon);

    // R = pio1_6
    // G = pio1_7
    // B = pio1_4
    //
    // on = low, off = high

    let pins = hal::Pins::take().unwrap();
    let mut red = pins
        .pio1_6
        .into_gpio_pin(&mut iocon, &mut gpio)
        .into_output(hal::drivers::pins::Level::High); // start turned off

    hal::ClockRequirements::default()
        // .support_usbfs()
        .system_frequency(12.MHz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .unwrap();

    loop {
        for _ in 0..10_000 {
            red.set_low().unwrap();
        }
        iprintln!(stim, "led on");
        // hprintln!("printed led on");

        for _ in 0..10_000 {
            red.set_high().unwrap();
        }
        iprintln!(stim, "led off");
    }
}
