use crate::soc::types::Soc as SocT;
use crate::types::Soc;

pub type CcidClass = usbd_ccid::Ccid<
    <SocT as Soc>::UsbBus,
    apdu_dispatch::interchanges::Contact,
    {apdu_dispatch::interchanges::SIZE},
>;
pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, <SocT as Soc>::UsbBus>;
// pub type KeyboardClass = usbd_hid::hid_class::HIDClass<'static, <SocT as Soc>::UsbBus>;
pub type SerialClass = usbd_serial::SerialPort<'static, <SocT as Soc>::UsbBus>;

type Usbd = usb_device::device::UsbDevice<'static, <SocT as Soc>::UsbBus>;

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

pub struct UsbInit {
	pub usb_classes: Option<UsbClasses>,
	pub apdu_dispatch: apdu_dispatch::dispatch::ApduDispatch,
	pub ctaphid_dispatch: ctaphid_dispatch::dispatch::Dispatch
}
