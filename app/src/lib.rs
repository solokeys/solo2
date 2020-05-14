#![no_std]

// panic handler, depending on debug/release build
// BUT: need to run in release anyway, to have USB work
// #[cfg(debug_assertions)]
// use panic_semihosting as _;
// #[cfg(not(debug_assertions))]
use panic_halt as _;

use core::convert::TryInto;

// board support package
#[cfg(not(any(feature = "board-lpcxpresso", feature = "board-prototype")))]
compile_error!("Please select one of the board support packages.");

#[cfg(feature = "board-lpcxpresso")]
pub use lpcxpresso55 as board;

#[cfg(feature = "board-prototype")]
pub use prototype_bee as board;

use c_stubs as _;

// re-exports for convenience
pub use board::hal;
pub use board::rt::entry;


pub mod types;
use types::{
    EnabledUsbPeripheral,
    ExternalStorage,
    VolatileStorage,
    Store,
};

//
// Board Initialization
//

use hal::drivers::{
    flash::FlashGordon,
    pins,
    Timer,
    UsbBus,
};
use interchange::Interchange;
use usbd_ccid::Ccid;
use usbd_ctaphid::CtapHid;
// use usbd_ctaphid::insecure::InsecureRamAuthenticator;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
// bring traits in scope
use hal::prelude::*;

// // filesystem starting at 320KB
// // this needs to be synchronized with contents of `memory.x`
// const FS_BASE: usize = 0x50_000;

// TODO: move board-specifics to BSPs
#[cfg(feature = "board-lpcxpresso")]
pub fn init_board(device_peripherals: hal::raw::Peripherals, core_peripherals: rtfm::Peripherals) -> (
    types::Authenticator,
    types::CcidClass,
    types::CryptoService,
    types::CtapHidClass,
    types::Piv,
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

    // let clocks = hal::ClockRequirements::default()
    //     #[cfg(not(feature = "highspeed"))]
    //     .support_usbfs()
    //     #[cfg(feature = "highspeed")]
    //     .support_usbhs()
    //     .system_frequency(96.mhz())
    //     .configure(&mut anactrl, &mut pmc, &mut syscon)
    //     .expect("Clock configuration failed");

    #[cfg(not(feature = "highspeed"))]
    let clocks = hal::ClockRequirements::default()
        .support_usbfs()
        .system_frequency(96.mhz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .expect("Clock configuration failed");

    #[cfg(feature = "highspeed")]
    let clocks = hal::ClockRequirements::default()
        .support_usbhs()
        .system_frequency(96.mhz())
        .configure(&mut anactrl, &mut pmc, &mut syscon)
        .expect("Clock configuration failed");

    #[cfg(feature = "highspeed")]
    let mut delay_timer = Timer::new(hal.ctimer.0.enabled(&mut syscon));

    #[cfg(feature = "highspeed")]
    let usbd = hal.usbhs.enabled_as_device(
        &mut anactrl,
        &mut pmc,
        &mut syscon,
        &mut delay_timer,
        clocks.support_usbhs_token().unwrap(),
    );

    #[cfg(not(feature = "highspeed"))]
    let usbd = hal.usbfs.enabled_as_device(
        &mut anactrl,
        &mut pmc,
        &mut syscon,
        clocks.support_usbfs_token().unwrap(),
    );

    let _: EnabledUsbPeripheral = usbd;
    // ugh, what's the nice way?
    static mut USB_BUS: Option<usb_device::bus::UsbBusAllocator<UsbBus<EnabledUsbPeripheral>>> = None;
    unsafe { USB_BUS = Some(hal::drivers::UsbBus::new(usbd, usb0_vbus_pin)); }
    let usb_bus = unsafe { USB_BUS.as_ref().unwrap() };

    // use littlefs2::fs::{Filesystem, FilesystemWith};
    // let mut alloc = Filesystem::allocate();
    // let mut fs = match FilesystemWith::mount(&mut alloc, &mut storage) {
    //     Ok(fs) => fs,
    //     Err(_) => {
    //         Filesystem::format(&mut storage).expect("format failed");
    //         FilesystemWith::mount(&mut alloc, &mut storage).unwrap()
    //     }
    // };

    let rng = hal.rng.enabled(&mut syscon);

    static mut CRYPTO_REQUESTS: trussed::pipe::RequestPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    static mut CRYPTO_REPLIES: trussed::pipe::ReplyPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    let mut client_id = littlefs2::path::PathBuf::new();
    client_id.push(b"fido2\0".try_into().unwrap());
    let (service_endpoint, client_endpoint) = trussed::pipe::new_endpoints(
        unsafe { &mut CRYPTO_REQUESTS },
        unsafe { &mut CRYPTO_REPLIES },
        client_id,
    );

    // static mut INTERNAL_STORAGE: InternalStorage = InternalStorage::new();
    // static mut INTERNAL_FS_ALLOC: Option<Allocation<InternalStorage>> = None;
    // unsafe { INTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }
    //
    use littlefs2::fs::{Allocation, Filesystem};

    let flash = hal.flash.enabled(&mut syscon);
    static mut INTERNAL_STORAGE: Option<FlashGordon> = None;
    unsafe { INTERNAL_STORAGE = Some(hal::drivers::flash::FlashGordon::new(flash)); }
    static mut INTERNAL_FS_ALLOC: Option<Allocation<FlashGordon>> = None;
    unsafe { INTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }

    static mut EXTERNAL_STORAGE: ExternalStorage = ExternalStorage::new();
    static mut EXTERNAL_FS_ALLOC: Option<Allocation<ExternalStorage>> = None;
    unsafe { EXTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }

    static mut VOLATILE_STORAGE: VolatileStorage = VolatileStorage::new();
    static mut VOLATILE_FS_ALLOC: Option<Allocation<VolatileStorage>> = None;
    unsafe { VOLATILE_FS_ALLOC = Some(Filesystem::allocate()); }


    let store = Store::claim().unwrap();
    store.mount(
        unsafe { INTERNAL_FS_ALLOC.as_mut().unwrap() },
        // unsafe { &mut INTERNAL_STORAGE },
        unsafe { INTERNAL_STORAGE.as_mut().unwrap() },
        unsafe { EXTERNAL_FS_ALLOC.as_mut().unwrap() },
        unsafe { &mut EXTERNAL_STORAGE },
        unsafe { VOLATILE_FS_ALLOC.as_mut().unwrap() },
        unsafe { &mut VOLATILE_STORAGE },
        // to trash existing data, set to true
        cfg!(feature = "format-storage")
    ).unwrap();

    // // just testing, remove again obviously
    // use trussed::store::Store as _;
    // let tmp_file = b"tmp.file\0".try_into().unwrap();
    // store.ifs().write(tmp_file, b"test data").unwrap();
    // let data: heapless::Vec<_, heapless::consts::U64> = store.ifs().read(tmp_file).unwrap();
    // cortex_m_semihosting::hprintln!("data: {:?}", &data).ok();

    let mut trussed = trussed::service::Service::new(rng, store);

    assert!(trussed.add_endpoint(service_endpoint).is_ok());

    let syscaller = trussed::client::TrussedSyscall::default();
    let crypto_client = trussed::client::Client::new(client_endpoint, syscaller);

    static mut AUTHNR_REQUESTS: ctap_types::rpc::RequestPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    static mut AUTHNR_RESPONSES: ctap_types::rpc::ResponsePipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    let (transport_pipe, authenticator_pipe) = ctap_types::rpc::new_endpoints(
        unsafe { &mut AUTHNR_REQUESTS },
        unsafe { &mut AUTHNR_RESPONSES },
    );

    // static mut PIV_REQUESTS: ctap_types::rpc::RequestPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    // static mut PIV_RESPONSES: ctap_types::rpc::ResponsePipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    // let (ccid_pipe, piv_pipe) = ctap_types::rpc::new_endpoints(
    //     unsafe { &mut PIV_REQUESTS },
    //     unsafe { &mut PIV_RESPONSES },
    // );

    let authnr = fido_authenticator::Authenticator::new(
        crypto_client, authenticator_pipe,
        fido_authenticator::SilentAuthenticator {},
        );

    // setup PIV
    let (requester, responder) =
        usbd_ccid::types::ApduInterchange::claim()
        .expect("could not setup ApduInterchange");

    static mut PIV_TRUSSED_REQUESTS: trussed::pipe::RequestPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    static mut PIV_TRUSSED_REPLIES: trussed::pipe::ReplyPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
    let mut client_id = littlefs2::path::PathBuf::new();
    client_id.push(b"piv\0".try_into().unwrap());
    let (piv_service_endpoint, piv_client_endpoint) = trussed::pipe::new_endpoints(
        unsafe { &mut PIV_TRUSSED_REQUESTS },
        unsafe { &mut PIV_TRUSSED_REPLIES },
        client_id,
    );
    assert!(trussed.add_endpoint(piv_service_endpoint).is_ok());

    let syscaller = trussed::client::TrussedSyscall::default();
    let piv_trussed = trussed::client::Client::new(
        piv_client_endpoint,
        syscaller,
    );

    let piv = piv_card::App::new(
        piv_trussed,
        responder,
    );

    // our USB classes
    let ctaphid = CtapHid::new(usb_bus, transport_pipe);
    let ccid = Ccid::new(usb_bus, requester);//, ccid_pipe);
    let serial = usbd_serial::SerialPort::new(usb_bus);

    // our composite USB device
    let usbd = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x1209, 0xBEEE))
    // no longer need to fake it, see README.md for how to get PCSC
    // to identify us as a smartcard.
    // let usbd = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x072f, 0x90cc))
        .manufacturer("SoloKeys")
        .product("Solo 🐝")
        .serial_number("20/20")
        .device_release(0x0123)
        // #[cfg(feature = "highspeed")]
        .max_packet_size_0(64)
        .build();

    (authnr, ccid, trussed, ctaphid, piv, rgb, serial, usbd)
}

//
// Logging
//

use funnel::{funnel, Drain};
use rtfm::Mutex;

funnel!(NVIC_PRIO_BITS = hal::raw::NVIC_PRIO_BITS, {
    0: 2048,
    1: 1024,
    2: 512,
    3: 512,
    7: 8192,
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

pub fn drain_log_to_semihosting() {
    use cortex_m_semihosting::{hprint, hprintln};
    let drains = Drain::get_all();
    let mut buf = [0u8; 64];

    for (_, drain) in drains.iter().enumerate() {
        'l: loop {
            let n = drain.read(&mut buf).len();
            if n == 0 {
                break 'l;
            }
            match core::str::from_utf8(&buf[..n]) {
                Ok(string) => hprint!(string).ok(),
                Err(e) => hprintln!("ERROR {:?}", &e).ok(),
            };
        }
    }
}
