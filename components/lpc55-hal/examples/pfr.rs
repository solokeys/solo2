#![no_main]
#![no_std]
/// Simple example to measure the core clock frequency

extern crate panic_semihosting;  // 4004 bytes
// extern crate panic_halt; // 672 bytes

use cortex_m_semihosting::{heprint,heprintln};
use cortex_m_rt::entry;

use lpc55_hal as hal;
use hal::{
    prelude::*,
    peripherals::pfr::{KeyType, Cfpa},
};

macro_rules! dump_hex {
    ($array:expr, $length:expr ) => {

        heprint!("{:?} = ", stringify!($array)).unwrap();
        for i in 0..$length {
            heprint!("{:02X}", $array[i]).unwrap();
        }
        heprintln!("").unwrap();

    };
}


#[allow(dead_code)]
fn boot_to_bootrom() -> ! {
    // Best way to boot into MCUBOOT is to erase the first flash page before rebooting.
    use hal::traits::flash::WriteErase;
    let flash = unsafe { hal::peripherals::flash::Flash::steal() }.enabled(
        &mut unsafe {hal::peripherals::syscon::Syscon::steal()}
    );
    hal::drivers::flash::FlashGordon::new(flash).erase_page(0).ok();
    hal::raw::SCB::sys_reset()
}

fn dump_cfpa(cfpa: &Cfpa) {
    heprintln!("header = {:08X}", cfpa.header).ok();
    heprintln!("version = {:08X}", cfpa.version).ok();
    heprintln!("secureVersion = {:08X}", cfpa.secure_fw_version).ok();
    heprintln!("notSecureVersion = {:08X}", cfpa.ns_fw_version).ok();

    heprintln!("imageKeyRevoke = {:08X}", cfpa.image_key_revoke).ok();
    heprintln!("rotkhRevoke = {:08X}", cfpa.rotkh_revoke).ok();
    dump_hex!(cfpa.customer_data, 10);
}


#[entry]
fn main() -> ! {
    let hal = hal::new();

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;


    let clocks = hal::ClockRequirements::default()
        .system_frequency(96.MHz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .unwrap();

    dump_hex!(hal::uuid(), 16);
    heprintln!("chip revision: {}", hal::chip_revision()).ok();
    let mut pfr = hal.pfr.enabled(&clocks).unwrap();
    let mut cfpa = pfr.read_latest_cfpa( ).unwrap();
    heprintln!("CFPA:").ok();
    dump_cfpa(&cfpa);


    heprintln!("Increment the version and write back cfpa!").ok();
    cfpa.version = cfpa.version + 1;
    cfpa.secure_fw_version += 1;
    cfpa.ns_fw_version += 1;
    // increment a byte of customer data (with overflow)
    cfpa.customer_data[0] = ((1 + (cfpa.customer_data[0] as u16)) & 0xff) as u8;
    pfr.write_cfpa(&cfpa).unwrap();

    heprintln!("Rerun this program and check that Version, firmware versions, and custom data byte all increment.").ok();

    let cmpa = pfr.read_cmpa().unwrap();
    heprintln!("\r\nCMPA:").ok();
    heprintln!("boot_cfg = {:08X}", cmpa.boot_cfg).ok();
    heprintln!("usb.vid = {:08X}", cmpa.usb_vid).ok();
    heprintln!("usb.pid = {:08X}", cmpa.usb_pid).ok();
    heprintln!("secure_boot_cfg = {:08X}", cmpa.secure_boot_cfg).ok();
    dump_hex!(cmpa.rotkh, cmpa.rotkh.len());
    dump_hex!(cfpa.customer_data, 10);
    dump_hex!(cmpa.customer_data, cmpa.customer_data.len());

    heprintln!("\r\nKeyStore:").ok();
    let key_code = pfr.read_key_code(KeyType::User).unwrap();
    dump_hex!(key_code, key_code.len());

    let activation_code = pfr.read_activation_code().unwrap();
    dump_hex!(activation_code, activation_code.len());

    pfr.lock_all().unwrap();

    heprintln!("done.  Must reboot to see CFPA changes take effect.").ok();

    loop {
        // done, insert NOP to avoid optimization weirdness
        continue;
    }
}
