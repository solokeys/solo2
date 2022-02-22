#![no_main]
#![no_std]

extern crate panic_semihosting;
// extern crate panic_halt;
use cortex_m_rt::entry;
// use core::fmt::Write;

use hal::prelude::*;
use lpc55_hal as hal;

use hal::{
    drivers::{
        Pins,
        Serial,
    },
};

#[allow(unused_imports)]
use cortex_m_semihosting::{hprintln, dbg};
use nb::block;

#[entry]
fn main() -> ! {

    let hal = hal::new();

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;
    let mut iocon = hal.iocon.enabled(&mut syscon);

    let clocks = hal::ClockRequirements::default()
        .system_frequency(50.MHz())
        // .support_flexcomm()
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .unwrap();

    // cortex_m_semihosting::hprintln!("clocks = {:?}", &clocks).ok();

    let token = clocks.support_flexcomm_token().unwrap();

    let usart = hal.flexcomm.2.enabled_as_usart(&mut syscon, &token);

    let pins = Pins::take().unwrap();
    // TX/RX on mikro BUS of dev board
    let tx = pins.pio0_27.into_usart2_tx_pin(&mut iocon);
    let rx = pins.pio1_24.into_usart2_rx_pin(&mut iocon);

    let config = hal::drivers::serial::config::Config::default()
        .speed(19_200.Hz());
    hprintln!("config = {:?}", config).ok();

    let serial = Serial::new(usart, (tx, rx), config);

    let (mut tx, mut rx) = serial.split();

    // Very simple example: Connect tx and rx on the board, send and receive one byte

    let sent = b'+';

    // The `block!` macro makes an operation block until it finishes

    block!(tx.write(sent)).ok();
    hprintln!("sent").ok();

    block!(tx.flush()).ok();
    hprintln!("flushed").ok();

    let received = block!(rx.read()).unwrap();
    hprintln!("received").ok();

    assert_eq!(received, sent);
    hprintln!("equal").ok();

    loop { continue; }
}
