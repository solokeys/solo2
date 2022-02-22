#![no_main]
#![no_std]

extern crate panic_semihosting;
use cortex_m::asm;
use cortex_m_rt::entry;
use cortex_m_semihosting::{dbg, hprint, hprintln};

use lpc55_hal as hal;

// use serde::Serialize;
// use serde_cbor::Serializer;
// use serde_cbor::ser::SliceWrite;

// #[derive(Serialize)]
// struct User {
//     user_id: u32,
//     password_hash: [u8; 4],
// }

#[entry]
fn main() -> ! {
    hprint!("a").unwrap();
    hprint!("b").unwrap();
    hprintln!("c").unwrap();

    const UUID: *mut u32 = 0x0009_FC70 as *mut u32;
    // dbg!(UUID);
    let mut uuid: [u32; 4] = [0; 4];
    for i in 0..4 {
        uuid[i] = unsafe { dbg!(UUID.offset(i as isize).read_volatile()) };
    }
    // dbg!(uuid);

    let x = "hello!";
    dbg!(x);

    // let mut buf = [0u8; 30];
    // let writer = SliceWrite::new(&mut buf[..]);
    // let mut ser = Serializer::new(writer);
    // let user = User {
    //     user_id: 42,
    //     password_hash: [1, 2, 3, 4],
    // };
    // user.serialize(&mut ser).unwrap();
    // let writer = ser.into_inner();
    // let size = writer.bytes_written();

    // dbg!(size, buf);

    let peripherals = hal::raw::Peripherals::take().unwrap();
    // const SYSCON: *mut u32 =
    let device_id = peripherals.SYSCON.device_id0.read().bits();
    hprintln!("device_id0 = {:x?}", device_id).unwrap();
    let dieid = peripherals.SYSCON.dieid.read().bits();
    hprintln!("dieid = {:x?}", dieid).unwrap();
    dbg!(peripherals.SYSCON.dieid.read().rev_id().bits());
    dbg!(peripherals.SYSCON.dieid.read().mco_num_in_die_id().bits());
    let cpstat = peripherals.SYSCON.cpstat.read().bits();
    hprintln!("cpstat = {:x?}", cpstat).unwrap();

    #[allow(non_snake_case)]
    let DEVICE_ID0: *mut u32 = 0x4000_0ff8 as *mut u32;
    hprintln!("{:x?}", unsafe { DEVICE_ID0.read_volatile() }).unwrap();

    #[allow(non_snake_case)]
    let DIEID: *mut u32 = 0x4000_0ffc as *mut u32;
    hprintln!("{:x?}", unsafe { DIEID.read_volatile() }).unwrap();

    dbg!(peripherals.SYSCON.device_id0.read().rom_rev_minor().bits());

    dbg!("Entering the endless loop...");
    loop {
        asm::wfi();
    }
}
