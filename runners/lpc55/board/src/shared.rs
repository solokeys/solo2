use crate::hal;

pub struct Monotonic;
pub struct Reboot;

const CLOCK_FREQ: u32 = 96_000_000;

impl crate::traits::Monotonic for Monotonic {
    // intended to be: milliseconds
    type Instant = i32;//core::time::Duration;
    unsafe fn reset() {}
    fn ratio() -> rtic::Fraction {
        rtic::Fraction { numerator: CLOCK_FREQ/1000, denominator: 1 }
    }
    fn now() -> Self::Instant {
        let rtc = unsafe { crate::hal::raw::Peripherals::steal() }.RTC;
        let secs = rtc.count.read().bits() as i32;
        let ticks_32k = rtc.subsec.read().bits() as i32;
        secs*1000 + ((ticks_32k * 61)/2000)
    }
    fn zero() -> Self::Instant {
        Self::Instant::default()
    }
}

impl crate::traits::Reboot for Reboot {
    fn reboot() -> ! {
        hal::raw::SCB::sys_reset()
    }
    fn reboot_to_firmware_update() -> ! {
        hal::boot_to_bootrom()
    }
    fn reboot_to_firmware_update_destructive() -> ! {
        // Erasing the first flash page & rebooting will keep processor in bootrom persistently.
        // This is however destructive, as a valid firmware will need to be flashed.
        use hal::traits::flash::WriteErase;
        let flash = unsafe { hal::peripherals::flash::Flash::steal() }.enabled(
            &mut unsafe {hal::peripherals::syscon::Syscon::steal()}
        );
        hal::drivers::flash::FlashGordon::new(flash).erase_page(0).ok();
        hal::raw::SCB::sys_reset()
    }
    fn locked() -> bool {
        let seal = &unsafe { hal::raw::Peripherals::steal() }.FLASH_CMPA.sha256_digest;
        seal.iter().any(|word| word.read().bits() != 0)
    }
}

