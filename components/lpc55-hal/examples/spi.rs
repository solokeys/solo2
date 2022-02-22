#![no_main]
#![no_std]

// extern crate panic_semihosting;
extern crate panic_halt;
use cortex_m_rt::entry;
use core::{convert::TryFrom, fmt::Write};

use lpc55_hal as hal;

use hal::{
    drivers::{
        Pins,
        SpiMaster,
    },
    typestates::{
        pin::flexcomm::{
            NoMiso,
            NoCs,
        },
    },
    time::{Hertz, RateExtensions},
    traits::wg::spi::{
        Mode,
        Phase,
        Polarity,
    },
};

use ssd1306::{
    self,
    prelude::*,
};


#[entry]
fn main() -> ! {

    let hal = hal::new();

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;
    let mut gpio = hal.gpio.enabled(&mut syscon);
    let mut iocon = hal.iocon.enabled(&mut syscon);

    let clocks = hal::ClockRequirements::default()
        // .system_freq(96.mhz())
        // .support_flexcomm()
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .unwrap();

    let token = clocks.support_flexcomm_token().unwrap();

    // SPI8 is the high-speed SPI
    let spi = hal.flexcomm.8.enabled_as_spi(&mut syscon, &token);

    let pins = Pins::take().unwrap();

    let sck = pins.pio1_2.into_spi8_sck_pin(&mut iocon);
    let mosi = pins.pio0_26.into_spi8_mosi_pin(&mut iocon);
    // let miso = pins.pio1_3.into_spi8_miso_pin(&mut iocon);
    let miso = NoMiso;
    // let cs = pins.pio1_1.into_spi8_cs_pin(&mut iocon);
    let cs = NoCs;

    // try this: currently no way to use SWCLK pin
    // let danger = pins.pio0_11.into_usart6_rx_pin(&mut iocon);

    let spi_pins = (sck, mosi, miso, cs);

    let spi_mode = Mode {
        polarity: Polarity::IdleLow,
        phase: Phase::CaptureOnFirstTransition,
    };

    let spi = SpiMaster::new(spi, spi_pins, Hertz::try_from(100_u32.kHz()).unwrap(), spi_mode);

    let dc = pins.pio1_5.into_gpio_pin(&mut iocon, &mut gpio).into_output_high();

    // OLED
    let mut display: TerminalMode<_> = ssd1306::Builder::new()
        .size(DisplaySize::Display128x32)
        // .size(DisplaySize::Display70x40)  // <-- TODO
        // .with_rotation(DisplayRotation::Rotate90)
        .connect_spi(spi, dc).into();

    display.init().unwrap();
    display.clear().ok();

    loop {
        for c in 97..123 {
            let _ = display.write_str(unsafe { core::str::from_utf8_unchecked(&[c]) });
        }
        for c in 65..91 {
            let _ = display.write_str(unsafe { core::str::from_utf8_unchecked(&[c]) });
        }
    }
}
