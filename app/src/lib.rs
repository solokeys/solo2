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
// use fido_authenticator::{
//     Authenticator,
//     // OsToAuthnrMessages,
//     // AuthnrToOsMessages,
//     // AuthnrChannels,
//     // OsChannels,
// };

use littlefs2::{
    consts,
    io::{
        Error as FsError,
        Result as FsResult,
    },
};

use heapless::{
    consts::U16,
    i::Queue as ConstQueue,
    spsc::{Consumer, Producer, Queue},
};

pub mod types;
use types::{
    InternalStorage,
    ExternalStorage,
    VolatileStorage,
    FlashStorage,
};

//
// Board Initialization
//

use hal::drivers::{
    pins,
    UsbBus,
};
use usbd_ctaphid::CtapHid;
// use usbd_ctaphid::insecure::InsecureRamAuthenticator;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
// bring traits in scope
use hal::prelude::*;

// filesystem starting at 320KB
// this needs to be synchronized with contents of `memory.x`
const FS_BASE: usize = 0x50_000;

// impl From<hal::drivers::FlashGordon> for FlashStorage {
//     fn from(driver: hal::drivers::FlashGordon) -> Self {
//         Self { driver }
//     }
// }

// impl littlefs2::driver::Storage for FlashStorage {
//     const READ_SIZE: usize = 16;
//     const WRITE_SIZE: usize = 512;
//     const BLOCK_SIZE: usize = 512;
//     const BLOCK_COUNT: usize = 64;
//     type CACHE_SIZE = consts::U512;
//     type LOOKAHEADWORDS_SIZE = consts::U16;
//     type FILENAME_MAX_PLUS_ONE = consts::U256;
//     type PATH_MAX_PLUS_ONE = consts::U256;
//     type ATTRBYTES_MAX = consts::U1022;

//     // Read data from the storage device. Guaranteed to be called only with bufs of length a multiple of READ_SIZE.
//     fn read(&self, off: usize, buf: &mut [u8]) -> Result<usize, FsError> {
//         hprintln!("reading {} from offset {}", buf.len(), off).ok();
//         let mut addr = FS_BASE + off;
//         for chunk in buf.chunks_mut(Self::READ_SIZE) {
//             self.driver.read(addr, chunk);
//             addr += Self::READ_SIZE;
//         }
//         Ok(buf.len())
//     }
//     fn write(&mut self, off: usize, data: &[u8]) -> Result<usize, FsError> {
//         hprintln!("writing {} to offset {}", data.len(), off).ok();
//         let mut addr = FS_BASE + off;
//         for chunk in data.chunks(Self::WRITE_SIZE) {
//             self.driver.write(addr, chunk).unwrap();
//             addr += Self::WRITE_SIZE;
//         }
//         Ok(data.len())
//     }
//     fn erase(&mut self, off: usize, len: usize) -> Result<usize, FsError> {
//         hprintln!("erasing {} from offset {}", len, off).ok();
//         let mut addr = FS_BASE + off;
//         let pages = len / Self::BLOCK_SIZE;
//         for page in 0..pages {
//             self.driver.erase_page(addr >> 4).unwrap();
//             addr += Self::BLOCK_SIZE;
//         }
//         Ok(len)
//     }
// }

// TODO: move board-specifics to BSPs
#[cfg(feature = "board-lpcxpresso")]
pub fn init_board(device_peripherals: hal::raw::Peripherals, core_peripherals: rtfm::Peripherals) -> (
    types::Authenticator,
    types::CryptoService,
    types::CtapHidClass,
    board::led::Rgb,
    types::SerialClass,
    types::Usbd,
) {
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

    // let flash = hal.flash.enabled(&mut syscon);
    // let driver = hal::drivers::flash::FlashGordon::new(flash);
    // let mut storage = FlashStorage::from(driver);

    // use littlefs2::fs::{Filesystem, FilesystemWith};
    // let mut alloc = Filesystem::allocate();
    // let mut fs = match FilesystemWith::mount(&mut alloc, &mut storage) {
    //     Ok(fs) => fs,
    //     Err(_) => {
    //         Filesystem::format(&mut storage).expect("format failed");
    //         FilesystemWith::mount(&mut alloc, &mut storage).unwrap()
    //     }
    // };

    let mut rng = hal.rng.enabled(&mut syscon);

    static mut CRYPTO_REQUESTS: crypto_service::pipe::RequestPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    static mut CRYPTO_REPLIES: crypto_service::pipe::ReplyPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    let (service_endpoint, client_endpoint) = crypto_service::pipe::new_endpoints(
        unsafe { &mut CRYPTO_REQUESTS },
        unsafe { &mut CRYPTO_REPLIES },
        "fido2",
    );

    use littlefs2::fs::{Filesystem, FilesystemAllocation, FilesystemWith};

    static mut INTERNAL_STORAGE: InternalStorage = InternalStorage::new();
    let internal_storage = unsafe { &mut INTERNAL_STORAGE };
    Filesystem::format(internal_storage).expect("could not format internal storage");
    static mut INTERNAL_FS_ALLOC: Option<FilesystemAllocation<InternalStorage>> = None;
    unsafe { INTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }
    let internal_fs_alloc = unsafe { INTERNAL_FS_ALLOC.as_mut().unwrap() };
    let ifs = FilesystemWith::mount(internal_fs_alloc, internal_storage)
        .expect("could not mount internal storage");

    static mut EXTERNAL_STORAGE: ExternalStorage = ExternalStorage::new();
    let external_storage = unsafe { &mut EXTERNAL_STORAGE };
    Filesystem::format(external_storage).expect("could not format external storage");
    static mut EXTERNAL_FS_ALLOC: Option<FilesystemAllocation<ExternalStorage>> = None;
    unsafe { EXTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }
    let external_fs_alloc = unsafe { EXTERNAL_FS_ALLOC.as_mut().unwrap() };
    let efs = FilesystemWith::mount(external_fs_alloc, external_storage)
        .expect("could not mount internal storage");

    static mut VOLATILE_STORAGE: VolatileStorage = VolatileStorage::new();
    let volatile_storage = unsafe { &mut VOLATILE_STORAGE };
    Filesystem::format(volatile_storage).expect("could not volatile internal storage");
    static mut VOLATILE_FS_ALLOC: Option<FilesystemAllocation<VolatileStorage>> = None;
    unsafe { VOLATILE_FS_ALLOC = Some(Filesystem::allocate()); }
    let volatile_fs_alloc = unsafe { VOLATILE_FS_ALLOC.as_mut().unwrap() };
    let vfs = FilesystemWith::mount(volatile_fs_alloc, volatile_storage)
        .expect("could not mount volatile storage");

    let mut crypto_service = crypto_service::service::Service::new(
        rng, ifs, efs, vfs).expect("service init worked");
    assert!(crypto_service.add_endpoint(service_endpoint).is_ok());

    let syscaller = types::CryptoSyscall::default();
    let mut crypto_client = crypto_service::client::Client::new(client_endpoint, syscaller);

    static mut AUTHNR_REQUESTS: ctap_types::rpc::RequestPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    static mut AUTHNR_RESPONSES: ctap_types::rpc::ResponsePipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    let (transport_pipe, authenticator_pipe) = ctap_types::rpc::new_endpoints(
        unsafe { &mut AUTHNR_REQUESTS },
        unsafe { &mut AUTHNR_RESPONSES },
    );

    let authnr = fido_authenticator::Authenticator::new(
        crypto_client, authenticator_pipe,
        fido_authenticator::SilentAuthenticator {},
        );

    // our USB classes
    let ctaphid = CtapHid::new(usb_bus, transport_pipe);
    let serial = usbd_serial::SerialPort::new(usb_bus);

    // our composite USB device
    let usbd = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x1209, 0xBEEE))
        .manufacturer("SoloKeys")
        .product("🐝")
        .serial_number("20/20")
        .device_release(0x0123)
        .build();

    (authnr, crypto_service, ctaphid, rgb, serial, usbd)
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
