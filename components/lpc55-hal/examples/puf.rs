#![no_main]
#![no_std]

use core::convert::TryInto;

extern crate panic_semihosting;  // 4004 bytes
// extern crate panic_halt; // 672 bytes

use cortex_m_rt::entry;
use cortex_m_semihosting::dbg;
use cortex_m_semihosting::heprintln;
use cortex_m_semihosting::heprint;

use lpc55_hal as hal;
use hal::prelude::*;

/// PUF error
#[derive(Debug)]
pub enum State {
    NotEnrolled,
    Enrolled = 0x7533ff04,
}

macro_rules! dump_hex {
    ($array:expr, $length:expr ) => {

        heprint!("{:?} = ", stringify!($array)).unwrap();
        for i in 0..$length {
            heprint!("{:02X}", $array[i]).unwrap();
        }
        heprintln!("").unwrap();

    };
}

#[entry]
fn main() -> ! {

    dbg!("Hello PUF");

    const PUF_STATE_FLASH: usize = 0x0006_0000; // 320kB offset

    // Get pointer to all device peripherals.
    let dp = hal::raw::Peripherals::take().unwrap();
    let mut syscon = hal::Syscon::from(dp.SYSCON);

    // Acquire PUF in an enabled state
    let puf = hal::Puf::from(dp.PUF).enabled(&mut syscon);

    let flash = hal::Flash::from(dp.FLASH).enabled(&mut syscon);
    let mut flash = hal::FlashGordon::new(flash);

    // Show PUF peripheral initial info
    dbg!(puf.version());
    dbg!(&puf);

    let mut buffer = [0u8; 16];
    flash.read(PUF_STATE_FLASH, &mut buffer);

    let state: u32 = u32::from_ne_bytes(buffer[0..4].try_into().unwrap() );
    let mut ac = [0u8; 1192];
    let mut kc1 = [0u8; 52];
    let mut kc2 = [0u8; 52];
    let mut kc3 = [0u8; 52];
    let mut kc4 = [0u8; 52];
    let mut check_buf = [0u8; 1192+52*4];

    if state != (State::Enrolled as u32) {
        dbg!("The is not yet enrolled. "); 
        dbg!("enrolling...");
        let mut write_buf = [0u8; 512];

        let puf_enrolled = puf.enroll(&mut ac).unwrap();

        dbg!(&puf_enrolled);
        
        dump_hex!(ac[..16], 16);
        dump_hex!(ac[1192-16..], 16);


        dbg!("Generate 2 IP-direct keys, and 2 normal keys.");
        puf_enrolled.generate_key(256, 0, &mut kc1).unwrap();
        puf_enrolled.generate_key(256, 0, &mut kc2).unwrap();
        puf_enrolled.generate_key(256, 1, &mut kc3).unwrap();
        puf_enrolled.generate_key(256, 2, &mut kc4).unwrap();

        // Print the 32 bit header + 32 bit of data for curiousity
        dump_hex!(kc1[0..8],8);
        dump_hex!(kc2[0..8],8);
        dump_hex!(kc3[0..8],8);
        dump_hex!(kc4[0..8],8);

        // Clear 3, 512-byte pages for segment: [ 16-byte header | 1192 byte AC ]
        for addr in (PUF_STATE_FLASH .. PUF_STATE_FLASH + 512*3).step_by(512) {
            flash.erase_page(addr >> 4).unwrap();
        }

        // write first 512-byte chunk
        write_buf[0..4].copy_from_slice(&(State::Enrolled as u32).to_ne_bytes());
        write_buf[16..].copy_from_slice(&ac[..496]);
        flash.write(PUF_STATE_FLASH + 0 , &write_buf).unwrap();

        // // write 2nd chunk
        write_buf.copy_from_slice(&ac[496..1008]);
        flash.write(PUF_STATE_FLASH + 512 , &write_buf).unwrap();

        // write 3rd chunk, with 4 KC's appended
        write_buf[0..184].copy_from_slice(&ac[1008..1192]);
        write_buf[184..236].copy_from_slice(&kc1);
        write_buf[236..288].copy_from_slice(&kc2);
        write_buf[288..340].copy_from_slice(&kc3);
        write_buf[340..392].copy_from_slice(&kc4);
        flash.write(PUF_STATE_FLASH + 1024 , &write_buf).unwrap();

        dbg!("Reading back...");
        flash.read(PUF_STATE_FLASH + 16, &mut check_buf);
        dump_hex!(check_buf[..16], 16);
        dump_hex!(check_buf[1192-16..], 16);
        for i in 0..ac.len() { assert!( ac[i] == check_buf[i] ) }

        for i in 0..kc1.len() { assert!( kc1[i] == check_buf[1192+52*0+i] ) }
        for i in 0..kc2.len() { assert!( kc2[i] == check_buf[1192+52*1+i] ) }
        for i in 0..kc3.len() { assert!( kc3[i] == check_buf[1192+52*2+i] ) }
        for i in 0..kc4.len() { assert!( kc4[i] == check_buf[1192+52*3+i] ) }

        dbg!("Now restart this program to derive the keys.");

    } else {
        dbg!("The device is already enrolled."); 
        flash.read(PUF_STATE_FLASH + 16, &mut check_buf);
        for i in 0..1192 {ac[i] = check_buf[i];}

        for i in 0..kc1.len() { kc1[i] = check_buf[1192+52*0+i]; }
        for i in 0..kc2.len() { kc2[i] = check_buf[1192+52*1+i]; }
        for i in 0..kc3.len() { kc3[i] = check_buf[1192+52*2+i]; }
        for i in 0..kc4.len() { kc4[i] = check_buf[1192+52*3+i]; }

        
        dump_hex!(ac[..16], 16);
        dump_hex!(ac[1192-16..], 16);

        let puf_started = puf.start(&ac).unwrap();

        dbg!("Started.");
        dbg!(&puf_started);

        dbg!("Loading AES and Prince Keys..");
        // Load into AES IP, and Prince IP for 3 address regions
        puf_started.get_key(hal::raw::puf::keyenable::KEY_A::AES, &kc1, &mut[0u8;0]).unwrap();
        puf_started.get_key(hal::raw::puf::keyenable::KEY_A::PRINCE0, &kc2, &mut[0u8;0]).unwrap();
        puf_started.get_key(hal::raw::puf::keyenable::KEY_A::PRINCE1, &kc2, &mut[0u8;0]).unwrap();
        puf_started.get_key(hal::raw::puf::keyenable::KEY_A::PRINCE2, &kc2, &mut[0u8;0]).unwrap();

        dbg!("Loading SW Keys..");
        let mut key1 = [0u8; 32];
        let mut key2 = [0u8; 32];
        puf_started.get_key(hal::raw::puf::keyenable::KEY_A::NONE, &kc3, &mut key1).unwrap();
        puf_started.get_key(hal::raw::puf::keyenable::KEY_A::NONE, &kc4, &mut key2).unwrap();

        dump_hex!(key1, 32);
        dump_hex!(key2, 32);

        dbg!(&puf_started);
        dbg!("Done");

    }




    dbg!("Looping");
    loop {
        
    }
}
