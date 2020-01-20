use crate::hal;
use hal::drivers::UsbBus;
use usbd_ctaphid::insecure::InsecureRamAuthenticator;

pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, InsecureRamAuthenticator, UsbBus>;
pub type SerialClass = usbd_serial::SerialPort<'static, UsbBus>;
pub type Usbd = usb_device::device::UsbDevice<'static, UsbBus>;

