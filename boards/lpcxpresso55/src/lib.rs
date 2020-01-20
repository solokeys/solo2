#![no_std]

pub use cortex_m_rt as rt;
pub use lpc55_hal as hal;

// pub mod button;
pub mod led;

// pub fn init_usbfs() {
//     let usb0_vbus_pin = pins::Pio0_22::take().unwrap()
//         .into_usb0_vbus_pin(&mut iocon);
//     let usbfsd = hal.usbfs.enabled_as_device(
//         &mut anactrl,
//         &mut pmc,
//         &mut syscon,
//         token,
//     );
// }
