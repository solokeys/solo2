#![no_std]
#![no_main]

use cortex_m_semihosting::{debug, hprintln};
extern crate panic_semihosting;
use cortex_m_rt::entry;

#[macro_use]
extern crate delog;

#[derive(Debug, Default)]
pub struct SemihostingFlusher {}

impl delog::Flusher for SemihostingFlusher {
    fn flush(&self, logs: &str) {
        cortex_m_semihosting::hprint!("{}", logs).ok();
    }
}

delog!(Delogger, 256, 256, SemihostingFlusher);

static SEMIHOSTING_FLUSHER: SemihostingFlusher = SemihostingFlusher {};

fn test_runs() {
    // do some serious work
    warn!("This is a warning");
    info!(target: "!", "This is an IMMEDIATE information");
    info!("jeez '{:02X}'", delog::hex_str!(&[0xa1u8, 0xfF, 0x03]));
    info!("heeb '{:#02X?}'", [0xa1u8, 0xfF, 0x03].as_ref());
    info!("heeg '{:02X?}'", [0xa1u8, 0xfF, 0x03].as_ref());

    // flush the logs
    Delogger::flush();
}

#[entry]
fn main() -> ! {

    Delogger::init(delog::LevelFilter::Info, &SEMIHOSTING_FLUSHER).ok();

    test_runs();

    hprintln!("All tests passed").ok();

    debug::exit(debug::EXIT_SUCCESS);

    loop { continue; }

}
