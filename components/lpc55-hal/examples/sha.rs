#![no_main]
#![no_std]

extern crate panic_semihosting;

use cortex_m_rt::entry;
use cortex_m_semihosting::dbg;

use lpc55_hal as hal;
use hal::traits::digest::{FixedOutput, Update};

#[entry]
fn main() -> ! {
    let mut cp = hal::raw::CorePeripherals::take().unwrap();
    cp.DWT.enable_cycle_counter();

    let dp = hal::raw::Peripherals::take().unwrap();

    let mut syscon = hal::Syscon::from(dp.SYSCON);
    let mut hashcrypt = hal::Hashcrypt::from(dp.HASHCRYPT).enabled(&mut syscon);

    // let msg: &[u8] = b"Be that word our sign of parting, bird or fiend! I shrieked upstarting.";

    const N: usize = 1025;
    let data = [37u8; N];
    let mut hw_cycles: u32 = 0;
    let mut sw_cycles: u32 = 0;

    // SHA-1
    for i in 0..N {
        let msg = &data[..i];

        let (hw_cyc, hw_result) = hal::count_cycles(|| {
            let mut hw_sha1 = hashcrypt.sha1();
            hw_sha1.update(msg);
            hw_sha1.finalize_fixed()
        });
        hw_cycles = hw_cyc;

        let (sw_cyc, sw_result) = hal::count_cycles(|| {
            let mut sw_sha1: sha1::Sha1 = Default::default();
            sw_sha1.update(msg);
            sw_sha1.finalize_fixed()
        });
        sw_cycles = sw_cyc;
        assert_eq!(hw_result, sw_result);
    }

    dbg!("SHA-1 checked for all sizes up to", N);
    dbg!("hw", hw_cycles, "sw", sw_cycles, "speedup", sw_cycles/hw_cycles);

    // SHA-256
    for i in 0..N {
        let msg = &data[..i];

        let (hw_cyc, hw_result) = hal::count_cycles(|| {
            let mut hw_sha256 = hashcrypt.sha256();
            hw_sha256.update(msg);
            hw_sha256.finalize_fixed()
        });
        hw_cycles = hw_cyc;

        let (sw_cyc, sw_result) = hal::count_cycles(|| {
            let mut sw_sha256: sha2::Sha256 = Default::default();
            sw_sha256.update(msg);
            sw_sha256.finalize_fixed()
        });
        sw_cycles = sw_cyc;

        assert_eq!(hw_result, sw_result);
    }

    dbg!("SHA-256 checked for all sizes up to", N);
    dbg!("hw", hw_cycles, "sw", sw_cycles, "speedup", sw_cycles/hw_cycles);

    dbg!("DONE");
    loop {
        continue;
    }
}
