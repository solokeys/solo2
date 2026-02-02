use crate::hal;

pub struct Reboot;

pub const CLOCK_FREQ: u32 = 96_000_000;

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
