#![no_std]

// panic handler, depending on debug/release build
// BUT: need to run in release anyway, to have USB work
// #[cfg(debug_assertions)]
use panic_semihosting as _;
// #[cfg(not(debug_assertions))]
// use panic_halt as _;

// board support package
#[cfg(not(any(feature = "board-lpcxpresso", feature = "board-prototype")))]
compile_error!("Please select one of the board support packages.");

#[cfg(feature = "board-lpcxpresso")]
pub use lpcxpresso55 as board;

#[cfg(feature = "board-prototype")]
pub use prototype_bee as board;

// re-exports for convenience
pub use board::hal;
pub use board::rt::entry;

use cortex_m_semihosting::hprintln;
use fido_authenticator::Authenticator;
use littlefs2::{
    consts,
    io::{
        Error as FsError,
        Result as FsResult,
    },
};

pub mod types;

//
// Board Initialization
//

use hal::drivers::{
    pins,
    UsbBus,
};
use usbd_ctaphid::CtapHid;
use usbd_ctaphid::insecure::InsecureRamAuthenticator;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
// bring traits in scope
use hal::prelude::*;

struct FlashStorage {
    driver: hal::drivers::FlashGordon,
}

// filesystem starting at 320KB
// this needs to be synchronized with contents of `memory.x`
const FS_BASE: usize = 0x50_000;

impl From<hal::drivers::FlashGordon> for FlashStorage {
    fn from(driver: hal::drivers::FlashGordon) -> Self {
        Self { driver }
    }
}

impl littlefs2::driver::Storage for FlashStorage {
    const READ_SIZE: usize = 16;
    const WRITE_SIZE: usize = 512;
    const BLOCK_SIZE: usize = 512;
    const BLOCK_COUNT: usize = 64;
    type CACHE_SIZE = consts::U512;
    type LOOKAHEADWORDS_SIZE = consts::U16;
    type FILENAME_MAX_PLUS_ONE = consts::U256;
    type PATH_MAX_PLUS_ONE = consts::U256;
    type ATTRBYTES_MAX = consts::U1022;

    // Read data from the storage device. Guaranteed to be called only with bufs of length a multiple of READ_SIZE.
    fn read(&self, off: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        // hprintln!("reading {} from offset {}", buf.len(), off).ok();
        let mut addr = FS_BASE + off;
        for chunk in buf.chunks_mut(Self::READ_SIZE) {
            self.driver.read(addr, chunk);
            addr += Self::READ_SIZE;
        }
        Ok(buf.len())
    }
    fn write(&mut self, off: usize, data: &[u8]) -> Result<usize, FsError> {
        // hprintln!("writing {} to offset {}", data.len(), off).ok();
        let mut addr = FS_BASE + off;
        for chunk in data.chunks(Self::WRITE_SIZE) {
            self.driver.write(addr, chunk).unwrap();
            addr += Self::WRITE_SIZE;
        }
        Ok(data.len())
    }
    fn erase(&mut self, off: usize, len: usize) -> Result<usize, FsError> {
        // hprintln!("erasing {} from offset {}", len, off).ok();
        let mut addr = FS_BASE + off;
        let pages = len / Self::BLOCK_SIZE;
        for page in 0..pages {
            self.driver.erase_page(addr >> 4).unwrap();
            addr += Self::BLOCK_SIZE;
        }
        Ok(len)
    }
}

// TODO: move board-specifics to BSPs
#[cfg(feature = "board-lpcxpresso")]
pub fn init_board(device_peripherals: hal::raw::Peripherals, core_peripherals: rtfm::Peripherals)
    -> (types::CtapHidClass, board::led::Rgb, types::SerialClass, types::Usbd)
{
    let hal = hal::Peripherals::from((device_peripherals, core_peripherals));

    let mut anactrl = hal.anactrl;
    let mut pmc = hal.pmc;
    let mut syscon = hal.syscon;

    let mut gpio = hal.gpio.enabled(&mut syscon);
    let mut iocon = hal.iocon.enabled(&mut syscon);

    let rgb = board::led::init_leds(
        pins::Pio1_4::take().unwrap(),
        pins::Pio1_6::take().unwrap(),
        pins::Pio1_7::take().unwrap(),
        &mut iocon, &mut gpio,
    );

    let usb0_vbus_pin = pins::Pio0_22::take().unwrap()
        .into_usb0_vbus_pin(&mut iocon);

    iocon.disabled(&mut syscon).release(); // save the environment :)

    let clocks = hal::ClockRequirements::default()
        .support_usbfs()
        .system_frequency(96.mhz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .expect("Clock configuration failed");

    let token = clocks.support_usbfs_token().unwrap();

    let usbfsd = hal.usbfs.enabled_as_device(
        &mut anactrl,
        &mut pmc,
        &mut syscon,
        token,
    );

    // ugh, what's the nice way?
    static mut USB_BUS: Option<usb_device::bus::UsbBusAllocator<UsbBus>> = None;
    unsafe { USB_BUS = Some(hal::drivers::UsbBus::new(usbfsd, usb0_vbus_pin)); }
    let usb_bus = unsafe { USB_BUS.as_ref().unwrap() };

    let flash = hal.flash.enabled(&mut syscon);
    let driver = hal::drivers::flash::FlashGordon::new(flash);
    let mut storage = FlashStorage::from(driver);

    use littlefs2::fs::{Filesystem, FilesystemWith};
    let mut alloc = Filesystem::allocate();
    let mut fs = match FilesystemWith::mount(&mut alloc, &mut storage) {
        Ok(fs) => fs,
        Err(_) => {
            Filesystem::format(&mut storage).expect("format failed");
            FilesystemWith::mount(&mut alloc, &mut storage).unwrap()
        }
    };

    let mut rng = hal.rng.enabled(&mut syscon);

    static mut AUTHENTICATOR: Option<Authenticator<
        'static, 'static,
        hal::Rng<hal::typestates::init_state::Enabled>,
        FlashStorage
    >> = None;
    unsafe { AUTHENTICATOR = Some(
        Authenticator::init(fs, rng)
    ) }

    // as above i guess
    static mut INSECURE_AUTHENTICATOR: Option<InsecureRamAuthenticator> = None;
    unsafe { INSECURE_AUTHENTICATOR = Some(InsecureRamAuthenticator::default()); }
    let authenticator = unsafe { INSECURE_AUTHENTICATOR.as_mut().unwrap() };

    // our USB classes
    let ctaphid = CtapHid::new(usb_bus, authenticator);
    let serial = usbd_serial::SerialPort::new(usb_bus);
    // let serial = usbd_serial::CdcAcmClass::new(usb_bus, 64);

    // our composite USB device
    let usbd = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x1209, 0xBEEE))
        .manufacturer("SoloKeys")
        .product("🐝")
        .serial_number("20/20")
        .device_release(0x0123)
        .build();

    (ctaphid, rgb, serial, usbd)
}

//
// Logging
//

use funnel::{funnel, Drain};
use rtfm::Mutex;

funnel!(NVIC_PRIO_BITS = hal::raw::NVIC_PRIO_BITS, {
    1: 1024,
    2: 512,
    3: 512,
});

pub fn drain_log_to_serial(mut serial: impl Mutex<T = types::SerialClass>) {
    let mut buf = [0u8; 64];

    let drains = Drain::get_all();

    for (_, drain) in drains.iter().enumerate() {
        'l: loop {
            let n = drain.read(&mut buf).len();
            if n == 0 {
                break 'l;
            }

            // cortex_m_semihosting::hprintln!("found {} bytes to log", n).ok();

            // serial.lock(|serial: &mut types::SerialClass| {
            //     match serial.write_packet(&buf[..n]) {
            //         Ok(count) => {
            //             cortex_m_semihosting::hprintln!("wrote {} to serial", count).ok();
            //             // cortex_m_semihosting::hprintln!("namely {:?}", &buf[..n]).ok();
            //         },
            //         Err(err) => {
            //             // not much we can do
            //             cortex_m_semihosting::hprintln!("error {:?} to serial wanting {}", err, n).ok();
            //             // cortex_m_semihosting::hprintln!("namely {:?}", &buf[..n]).ok();
            //         },
            //     }
            // });

            //     // not much we can do
            //     serial.flush().ok();
            // });
            serial.lock(|serial: &mut types::SerialClass| {
                // let mut read_buf = [0u8; 64];
                // match serial.read(&mut read_buf[..]) {
                //     Ok(n) => {
                //         cortex_m_semihosting::hprintln!("got {:?} on serial", &read_buf[..n]).ok();
                //     },
                //     Err(err) => {
                //         cortex_m_semihosting::hprintln!("serial read error: {:?}", err).ok();
                //     }
                // };

                match serial.write(&buf[..n]) {
                    Ok(_count) => {
                        // cortex_m_semihosting::hprintln!("wrote {} to serial", count).ok();
                    },
                    Err(_err) => {
                        // not much we can do
                        // cortex_m_semihosting::hprintln!("error writing to serial {:?}", err).ok();
                    },
                }

                // not much we can do
                serial.flush().ok();
            });
        }
    }
}
