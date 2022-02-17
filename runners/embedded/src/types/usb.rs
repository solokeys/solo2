use crate::soc;

pub type CcidClass<'a> = usbd_ccid::Ccid<
    soc::types::UsbBus<'static>,
    apdu_dispatch::interchanges::Contact,
    {apdu_dispatch::interchanges::SIZE},
>;
pub type CtapHidClass<'a> = usbd_ctaphid::CtapHid<'a, soc::types::UsbBus<'static>>;
// pub type KeyboardClass<'a> = usbd_hid::hid_class::HIDClass<'a, soc::types::UsbBus<'static>>;
pub type SerialClass<'a> = usbd_serial::SerialPort<'a, soc::types::UsbBus<'static>>;

type Usbd<'a> = usb_device::device::UsbDevice<'a, soc::types::UsbBus<'static>>;

pub struct UsbClasses<'a> {
    pub usbd: Usbd<'a>,
    pub ccid: CcidClass<'a>,
    pub ctaphid: CtapHidClass<'a>,
    // pub keyboard: KeyboardClass<'a>,
    pub serial: SerialClass<'a>,
}

impl<'a> UsbClasses<'a> {
    pub fn new(usbd: Usbd<'a>, ccid: CcidClass<'a>, ctaphid: CtapHidClass<'a>, serial: SerialClass<'a>) -> Self {
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

