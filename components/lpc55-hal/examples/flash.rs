#![no_main]
#![no_std]

extern crate panic_semihosting;
use cortex_m_rt::entry;
use cortex_m_semihosting::{dbg, hprintln};

use lpc55_hal as hal;
use hal::prelude::*;

#[repr(C)]
#[allow(dead_code)]
enum FlashCommands {
    Init = 0x0,
    PowerDown = 0x1,
    SetReadMode = 0x2,
    ReadSingleWord = 0x3,
    EraseRange = 0x4,
    BlankCheck = 0x5,
    MarginCheck = 0x6,
    Checksum = 0x7,
    Write = 0x8,
    WriteProg = 0xA,
    Program = 0xC,
    ReportEcc= 0xD,
}

#[entry]
fn main() -> ! {

    let hal = hal::new();

    // dbg!(hal.FLASH_CMPA.boot_cfg.read().bits());
    // dbg!(hal.FLASH_CMPA.boot_cfg.read().boot_speed().is_value_0());
    // dbg!(hal.FLASH_CMPA.boot_cfg.read().boot_speed().is_value_1());
    // dbg!(hal.FLASH_CMPA.boot_cfg.read().boot_speed().is_value_2());
    // dbg!(hal.FLASH_CMPA.usb_id.read().usb_vendor_id().bits());
    // dbg!(hal.FLASH_CMPA.usb_id.read().usb_product_id().bits());

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;

    hal::ClockRequirements::default()
        .system_frequency(12.MHz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .unwrap();

    let flash = hal.flash.enabled(&mut syscon);

    let mut flash = hal::FlashGordon::new(flash);

    // let flash = flash.release();

    // fn show_status(flash: &hal::raw::FLASH) {
    //     dbg!(flash.int_status.read().done().bit());
    //     dbg!(flash.int_status.read().ecc_err().bit());
    //     dbg!(flash.int_status.read().err().bit());
    //     dbg!(flash.int_status.read().fail().bit());
    // }

    // // show_status(&flash);

    // flash.event.write(|w| w.rst().set_bit());
    // // seems immediate
    // while flash.int_status.read().done().bit_is_clear() {}
    // // first thing to check! illegal command
    // debug_assert!(flash.int_status.read().err().bit_is_clear());
    // // first thing to check! legal command failed
    // debug_assert!(flash.int_status.read().fail().bit_is_clear());
    // // show_status(&flash);

    // const READ_SIZE: usize = 16;
    // fn read_native(addr: usize, flash: &hal::raw::FLASH) -> [u8; READ_SIZE] {
    //     let addr = addr as u32;
    //     debug_assert!(addr & (READ_SIZE as u32 - 1) == 0);
    //     let mut physical_word =  [0u8; 16];

    //     flash.starta.write(|w| unsafe { w.starta().bits(addr >> 4) } );
    //     flash.cmd.write(|w| unsafe { w.bits(FlashCommands::ReadSingleWord as u32) });
    //     while flash.int_status.read().done().bit_is_clear() {}
    //     debug_assert!(flash.int_status.read().err().bit_is_clear());
    //     debug_assert!(flash.int_status.read().fail().bit_is_clear());

    //     for (i, chunk) in physical_word.chunks_mut(4).enumerate() {
    //         chunk.copy_from_slice(&flash.dataw[i].read().bits().to_ne_bytes());
    //     }
    //     physical_word
    // }

    // // non-secure: 0x0000_0000 to 0x0009_FFFF;
    // const WHERE: u32 = 0x0004_0000; // 256kB offset
    // let page = WHERE >> 4;
    // // flash.starta.write(|w| unsafe { w.starta().bits(WHERE >> 4) } );
    // // flash.cmd.write(|w| unsafe { w.bits(FlashCommands::ReadSingleWord as u32) });
    // // while flash.int_status.read().done().bit_is_clear() {}
    // // debug_assert!(flash.int_status.read().err().bit_is_clear());
    // // debug_assert!(flash.int_status.read().fail().bit_is_clear());
    // // // show_status(&flash);
    // // dbgx!(flash.dataw[0].read().bits());
    // // dbgx!(flash.dataw[1].read().bits());
    // // dbgx!(flash.dataw[2].read().bits());
    // // dbgx!(flash.dataw[3].read().bits());
    // dbg!(read_native(WHERE as usize, &flash));

    // // now erase the page
    // flash.stopa.write(|w| unsafe { w.stopa().bits(WHERE >> 4) } );
    // flash.cmd.write(|w| unsafe { w.bits(FlashCommands::EraseRange as u32) });
    // while flash.int_status.read().done().bit_is_clear() {}
    // debug_assert!(flash.int_status.read().err().bit_is_clear());
    // debug_assert!(flash.int_status.read().fail().bit_is_clear());

    // // check it's erased
    // flash.cmd.write(|w| unsafe { w.bits(FlashCommands::BlankCheck as u32) });
    // while flash.int_status.read().done().bit_is_clear() {}
    // debug_assert!(flash.int_status.read().err().bit_is_clear());
    // debug_assert!(flash.int_status.read().fail().bit_is_clear());
    // hprintln!("Page {:#x} ({}) is blank!", page, page);

    // // write some stuff
    // let data: [u32; 4] = [0x7, 0x2, 0x3, 0x4];
    // for i in 0..=3 {
    //     flash.dataw[i].write(|w| unsafe { w.bits(data[i]) } );
    // }
    // flash.cmd.write(|w| unsafe { w.bits(FlashCommands::Write as u32) });
    // while flash.int_status.read().done().bit_is_clear() {}
    // debug_assert!(flash.int_status.read().err().bit_is_clear());
    // debug_assert!(flash.int_status.read().fail().bit_is_clear());

    // // program it
    // flash.cmd.write(|w| unsafe { w.bits(FlashCommands::Program as u32) });
    // while flash.int_status.read().done().bit_is_clear() {}
    // debug_assert!(flash.int_status.read().err().bit_is_clear());
    // debug_assert!(flash.int_status.read().fail().bit_is_clear());


    // let x: u8 = unsafe { core::ptr::read_volatile(0x0004_0000 as *const u8) } ;
    // hprintln!("{:x}", x).ok();
    // let x: u32 = unsafe { core::ptr::read_volatile(0x0004_0004 as *const u32) } ;
    // hprintln!("{:x}", x).ok();

    dbg!("before erasing");
    hprintln!("{:#034x}", flash.read_u128(0x4_0000)).ok();
    const WHERE: usize = 0x0004_0000; // 256kB offset

    dbg!("after erasing");
    flash.erase_page(WHERE >> 4).unwrap();
    hprintln!("{:#034x}", flash.read_u128(0x4_0000)).ok();

    dbg!("after writing");
    flash.write_u32(WHERE, 0x1234_5678).unwrap();
    hprintln!("{:#034x}", flash.read_u128(0x4_0000)).ok();

    dbg!("after erasing again");
    flash.erase_page(WHERE >> 4).unwrap();
    hprintln!("{:#034x}", flash.read_u128(0x4_0000)).ok();

    dbg!("after writing with offset 4");
    flash.write_u32(WHERE + 4, 0x1234_5678).unwrap();
    hprintln!("{:#034x}", flash.read_u128(0x4_0000)).ok();

    hprintln!("{:#034x}", flash.read_u128(0x4_0010)).ok();
    hprintln!("{:#034x}", flash.read_u128(0x4_0020)).ok();


    let mut read_buf = [0u8; 16];
    flash.read(WHERE, &mut read_buf);
    // dbg!(read_buf);

    flash.erase_page(0x4_0200).unwrap();
    hprintln!("supposedly erased").ok();
    // dbg!(flash.status());
    flash.read(WHERE, &mut read_buf);
    // dbg!(read_buf);

    let data: [u8; 4] = [0x7, 0x2, 0x3, 0x4];
    let mut buf = [0u8; 512];
    buf[..4].copy_from_slice(&data);

    flash.write_native(WHERE, &generic_array::GenericArray::from_slice(&buf)).unwrap();
    buf[0] = 37;
    // // buf[3] = 37;
    flash.write_native(WHERE, &generic_array::GenericArray::from_slice(&buf)).unwrap();
    flash.write_u8(0x4_000F, 69).ok();
    flash.read(WHERE, &mut read_buf);
    // dbg!(read_buf);

    // flash.clear_page_register();
    // flash.just_program_at(0x4_0200).unwrap();
    flash.erase_page(0x4_0200 >> 4).unwrap();
    // flash.read(0x4_0200, &mut read_buf);
    // dbg!(read_buf);
    // flash.write_u32(0x4_0200, 32).ok();
    // hprintln!("{:#x}", flash.read_u128(0x4_0200)).ok();
    // flash.read(0x4_0200, &mut read_buf);
    // dbg!(read_buf);
    // // flash.write_u8(0x4_0206, 64).ok();
    // flash.write_u32(0x4_0204, 128).ok();
    // hprintln!("{:#x}", flash.read_u128(0x4_0200)).ok();
    // flash.read(0x4_0200, &mut read_buf);
    // dbg!(read_buf);
    // // flash.read(0x4_0210, &mut read_buf);
    // // dbg!(read_buf);

    hprintln!("{:#034x}", flash.read_u128(0x4_0200)).ok();
    hprintln!("{:#034x}", flash.read_u128(0x4_0210)).ok();
    hprintln!("{:#034x}", flash.read_u128(0x4_0220)).ok();

    flash.write_u128(0x4_0200, 0x1234567).unwrap();
    // hal::wait_at_least(1_000_000);
    flash.write_u128(0x4_0210, 0x7654321).unwrap();
    // hal::wait_at_least(1_000_000);
    // flash.write_u128(0x4_0200, 0x1234567).unwrap();

    hprintln!("{:#034x}", flash.read_u128(0x4_0200)).ok();
    hprintln!("{:#034x}", flash.read_u128(0x4_0210)).ok();
    hprintln!("{:#034x}", flash.read_u128(0x4_0220)).ok();


    hprintln!("loop-continue").ok();
    loop {
        continue;
    }

}
