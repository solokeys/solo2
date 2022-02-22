#![no_main]
#![no_std]

extern crate panic_semihosting;
use cortex_m::asm;
use cortex_m_rt::entry;
use cortex_m_semihosting::dbg;

use lpc55_hal as hal;
use hal::traits::rand_core::RngCore;

#[entry]
fn main() -> ! {
    // TODO: use hal::Peripherals
    let mut dp = hal::raw::Peripherals::take().unwrap();
    let mut cp = hal::raw::CorePeripherals::take().unwrap();

    cp.DWT.enable_cycle_counter();

    let before = hal::get_cycle_count();
    asm::nop();
    asm::nop();
    asm::nop();
    let after = hal::get_cycle_count();
    dbg!(after - before);
    // idbg!(after);

    let mut syscon = hal::Syscon::from(dp.SYSCON);
    dbg!(hal::get_cycle_count());

    // TODO: make this method generic over i (in this case, 2)
    dbg!(syscon.is_clock_enabled(&dp.RNG)); // seems default is: yes!
    syscon.disable_clock(&mut dp.RNG);
    dbg!(syscon.is_clock_enabled(&dp.RNG));
    syscon.enable_clock(&mut dp.RNG);
    dbg!(syscon.is_clock_enabled(&dp.RNG));

    // NB: if RNG clock were disabled, reads below would get stuck

    // raw access
    dbg!(dp.RNG.random_number.read().bits());

    // HAL access
    let mut rng = hal::Rng::from(dp.RNG).enabled(&mut syscon);
    let mut random_bytes = [0u8; 5];
    rng.fill_bytes(&mut random_bytes);
    dbg!(random_bytes);

    dbg!(rng.module_id());

    // let syscon = hal::syscon::SYSCON::new(dp.SYSCON);
    // dbg!(syscon.rev_id());

    loop {
        asm::wfi();
        // dbg!(rng.get_random_u32());
    }
}
