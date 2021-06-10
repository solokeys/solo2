use crate::hal;
use hal::drivers::{UsbBus};

#[cfg(not(feature = "usbfs-peripheral"))]
pub type EnabledUsbPeripheral = hal::peripherals::usbhs::EnabledUsbhsDevice;
#[cfg(feature = "usbfs-peripheral")]
pub type EnabledUsbPeripheral = hal::peripherals::usbfs::EnabledUsbfsDevice;

pub type CcidClass = usbd_ccid::Ccid<
    UsbBus<EnabledUsbPeripheral>,
    apdu_dispatch::interchanges::Contact,
    {apdu_dispatch::interchanges::SIZE},
>;
pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, UsbBus<EnabledUsbPeripheral>>;
// pub type KeyboardClass = usbd_hid::hid_class::HIDClass<'static, UsbBus<EnabledUsbPeripheral>>;
pub type SerialClass = usbd_serial::SerialPort<'static, UsbBus<EnabledUsbPeripheral>>;

type Usbd = usb_device::device::UsbDevice<'static, UsbBus<EnabledUsbPeripheral>>;

pub struct UsbClasses {
    pub usbd: Usbd,
    pub ccid: CcidClass,
    pub ctaphid: CtapHidClass,
    // pub keyboard: KeyboardClass,
    pub serial: SerialClass,
}

impl UsbClasses {
    pub fn new(usbd: Usbd, ccid: CcidClass, ctaphid: CtapHidClass, serial: SerialClass) -> Self {
        Self{ usbd, ccid, ctaphid, serial }
    }
    pub fn poll(&mut self) {
        self.ctaphid.check_for_app_response();
        self.ccid.check_for_app_response();
        self.usbd.poll(&mut [
            &mut self.ccid,
            &mut self.ctaphid,
            &mut self.serial,
        ]);
    }
}

