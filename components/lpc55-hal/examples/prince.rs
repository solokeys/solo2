#![no_main]
#![no_std]

extern crate panic_semihosting;
use cortex_m_rt::entry;

#[allow(unused_imports)]
use cortex_m_semihosting::{dbg, hprintln, hprint};

use lpc55_hal as hal;
use hal::prelude::*;

macro_rules! dump_hex {
    ($array:expr) => {

        hprint!("{:?} = ", stringify!($array)).unwrap();
        for i in 0..$array.len() {
            hprint!("{:02X}", $array[i]).unwrap();
        }
        hprintln!("").unwrap();

    };
}

#[entry]
fn main() -> ! {

    let hal = hal::new();

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;

    // prince region 2 (128KB)
    const DATA_ADDR: usize = 0x00080000 + 0;

    let _clocks = hal::ClockRequirements::default()
        .system_frequency(12.MHz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .unwrap();

    let flash = hal.flash.enabled(&mut syscon);
    let mut flash = hal::FlashGordon::new(flash);

    let mut rng = hal.rng.enabled(&mut syscon);

    let mut prince = hal.prince.enabled(&mut rng);

    prince.enable_all_region_2();


    hprintln!("writing AA's to flash data.").ok();

    flash.erase_page((DATA_ADDR/512) + 0).unwrap();
    flash.erase_page((DATA_ADDR/512) + 1).unwrap();

    prince.write_encrypted(|_prince| {
        let vector = [0xAA; 1024];
        flash.write(DATA_ADDR, &vector).unwrap();
    });


    hprintln!("Read bytes PRINCE ON:").ok();
    let mut buf = [0u8; 1024];

    for i in 0 .. buf.len() {
        let ptr = DATA_ADDR as *const u8;
        buf[i] = unsafe{*ptr.offset(i as isize)};
    }

    dump_hex!(&buf[0..32]);

    // Turn off PRINCE.
    prince.disable_all_region_2();

    for i in 0 .. buf.len() {
        let ptr = DATA_ADDR as *const u8;
        buf[i] = unsafe{*ptr.offset(i as isize)};
    }

    hprintln!("Read bytes PRINCE OFF:").ok();
    dump_hex!(&buf[0..32]);


    hprintln!("done.").ok();
    loop {
        continue;
    }

}
