use crate::hal;
use hal::drivers::UsbBus;
use littlefs2::{
    const_ram_storage,
    ram_storage,
};
use crypto_service::types::{LfsResult, LfsStorage};
use ctap_types::consts;
use fido_authenticator::SilentAuthenticator;
// use usbd_ctaphid::insecure::InsecureRamAuthenticator;

pub struct FlashStorage {
    pub driver: hal::drivers::FlashGordon,
}

pub type Authenticator = fido_authenticator::Authenticator<'static, CryptoSyscall, SilentAuthenticator>;

#[derive(Default)]
pub struct CryptoSyscall {}

impl crypto_service::pipe::Syscall for CryptoSyscall {
    fn syscall(&mut self) {
        rtfm::pend(hal::raw::Interrupt::OS_EVENT);
    }
}

const_ram_storage!(InternalStorage, 4096);
const_ram_storage!(ExternalStorage, 4096);
const_ram_storage!(VolatileStorage, 4096);

pub type CryptoService = crypto_service::Service<
    'static, 'static,
    hal::peripherals::rng::Rng<hal::Enabled>,
    InternalStorage,
    ExternalStorage,
    VolatileStorage,
>;

// pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, InsecureRamAuthenticator, UsbBus>;
pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, 'static, UsbBus>;

pub type SerialClass = usbd_serial::SerialPort<'static, UsbBus>;
// pub type SerialClass = usbd_serial::CdcAcmClass<'static, UsbBus>;
pub type Usbd = usb_device::device::UsbDevice<'static, UsbBus>;

