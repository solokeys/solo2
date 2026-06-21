use crate::hal;
use hal::drivers::UsbBus;

#[cfg(not(feature = "usbfs-peripheral"))]
pub type EnabledUsbPeripheral = hal::peripherals::usbhs::EnabledUsbhsDevice;
#[cfg(feature = "usbfs-peripheral")]
pub type EnabledUsbPeripheral = hal::peripherals::usbfs::EnabledUsbfsDevice;

pub type CcidClass = usbd_ccid::Ccid<
    'static,
    'static,
    UsbBus<EnabledUsbPeripheral>,
    { apdu_dispatch::interchanges::SIZE },
>;
pub type CtapHidClass = usbd_ctaphid::CtapHid<
    'static,
    'static,
    'static,
    UsbBus<EnabledUsbPeripheral>,
    { ctaphid_dispatch::DEFAULT_MESSAGE_SIZE },
>;
// pub type KeyboardClass = usbd_hid::hid_class::HIDClass<'static, UsbBus<EnabledUsbPeripheral>>;
pub type SerialClass = usbd_serial::SerialPort<'static, UsbBus<EnabledUsbPeripheral>>;

#[cfg(feature = "wallet")]
pub type WalletHidClass =
    wallet_app::usbd::WalletHid<'static, 'static, UsbBus<EnabledUsbPeripheral>>;

type Usbd = usb_device::device::UsbDevice<'static, UsbBus<EnabledUsbPeripheral>>;

pub struct UsbClasses {
    pub usbd: Usbd,
    pub ccid: CcidClass,
    pub ctaphid: CtapHidClass,
    // Wallet HID transport. Matched by host wallet host tools via the vendor
    // usage page (0xFFA0) on its HID report descriptor.
    #[cfg(feature = "wallet")]
    pub wallet_hid: WalletHidClass,
    // pub keyboard: KeyboardClass,
    // pub serial: SerialClass,
}

impl UsbClasses {
    pub fn new(
        usbd: Usbd,
        ccid: CcidClass,
        ctaphid: CtapHidClass,
        #[cfg(feature = "wallet")] wallet_hid: WalletHidClass,
    ) -> Self {
        Self {
            usbd,
            ccid,
            ctaphid,
            #[cfg(feature = "wallet")]
            wallet_hid,
        }
    }
    pub fn poll(&mut self) {
        self.ctaphid.check_for_app_response();
        self.ccid.check_for_app_response();
        #[cfg(feature = "wallet")]
        self.wallet_hid.check_for_app_response();
        self.usbd.poll(&mut [
            #[cfg(feature = "wallet")]
            &mut self.wallet_hid,
            &mut self.ccid,
            &mut self.ctaphid,
            // &mut self.serial,
        ]);
    }
}
