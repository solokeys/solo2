use crate::hal;
use hal::drivers::{UsbBus};

pub type EnabledUsbPeripheral = hal::peripherals::usbhs::EnabledUsbhsDevice;

pub type CcidClass = usbd_ccid::Ccid<UsbBus<EnabledUsbPeripheral>>;
// pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, InsecureRamAuthenticator, UsbBus>;
pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, UsbBus<EnabledUsbPeripheral>>;

pub type SerialClass = usbd_serial::SerialPort<'static, UsbBus<EnabledUsbPeripheral>>;
// pub type SerialClass = usbd_serial::CdcAcmClass<'static, UsbBus>;
type Usbd = usb_device::device::UsbDevice<'static, UsbBus<EnabledUsbPeripheral>>;

pub struct UsbWrapper {
    pub usbd: Usbd,
    pub ccid: CcidClass,
    pub ctaphid: CtapHidClass,
    pub serial: SerialClass,
}

impl UsbWrapper {
    pub fn new(usbd: Usbd, ccid: CcidClass, ctaphid: CtapHidClass, serial: SerialClass) -> Self {
        Self{
            usbd, ccid, ctaphid, serial
        }
    }
}

