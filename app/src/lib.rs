#![no_std]

// panic handler, depending on debug/release build
#[cfg(debug_assertions)]
use panic_semihosting as _;
#[cfg(not(debug_assertions))]
use panic_halt as _;

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

// type Ctap = usbd_ctaphid::CtapHid<'static, InsecureRamAuthenticator, UsbBus>;
// type Serial = usbd_serial::SerialPort<'static, hal::drivers::usbd::UsbBus>;
// type Usbd = usb_device::device::UsbDevice<'static, hal::drivers::usbd::UsbBus>;

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

    // // TODO: move to BSP
    // let mut _red_led = pins::Pio1_6::take().unwrap()
    //     .into_gpio_pin(&mut iocon, &mut gpio)
    //     .into_output(hal::drivers::pins::Level::High); // start turned off

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

    static mut USB_BUS: Option<usb_device::bus::UsbBusAllocator<UsbBus>> = None;
    unsafe {
        USB_BUS = Some(hal::drivers::UsbBus::new(usbfsd, usb0_vbus_pin));
    }

    let usb_bus = unsafe { USB_BUS.as_ref().unwrap() };

    // as above i guess
    static mut AUTHENTICATOR: Option<InsecureRamAuthenticator> = None;
    unsafe {
        AUTHENTICATOR = Some(InsecureRamAuthenticator::default());
    }
    let authenticator = unsafe { AUTHENTICATOR.as_mut().unwrap() };
    let mut ctaphid = CtapHid::new(usb_bus, authenticator);

    let mut serial = usbd_serial::SerialPort::new(usb_bus);

    let mut usbd = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x1209, 0xF1D0))
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

use funnel::{funnel, Drain, Logger};
use rtfm::Mutex;

funnel!(NVIC_PRIO_BITS = hal::raw::NVIC_PRIO_BITS, {
    1: 512,
    2: 512,
    3: 512,
});

// type Serial = usbd_serial::SerialPort<'static, hal::drivers::usbd::UsbBus>;

pub fn drain_log_to_serial(mut serial: impl Mutex<T = types::SerialClass>) {
    let mut buf = [0u8; 512];

    let drains = Drain::get_all();

    use hal::traits::wg::serial::Write;

    loop {
        for (i, drain) in drains.iter().enumerate() {
            'l: loop {
                let n = drain.read(&mut buf).len();
                if n == 0 {
                    break 'l;
                }

                serial.lock(|serial: &mut types::SerialClass| {
                    match serial.write(&buf[..n]) {
                        Ok(count) => {
                            // cortex_m_semihosting::hprintln!("wrote {} to serial", count).ok();
                        },
                        Err(err) => {
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
}
