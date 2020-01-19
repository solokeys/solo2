use crate::hal;
use hal::drivers::{
    pins,
    UsbBus,
};
use usbd_ctaphid::CtapHid;
use usbd_ctaphid::insecure::InsecureRamAuthenticator;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
// bring traits in scope
use hal::prelude::*;

pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, InsecureRamAuthenticator, UsbBus>;
pub type SerialClass = usbd_serial::SerialPort<'static, hal::drivers::usbd::UsbBus>;
pub type Usbd = usb_device::device::UsbDevice<'static, hal::drivers::usbd::UsbBus>;

