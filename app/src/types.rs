use crate::hal;
use hal::drivers::UsbBus;
use littlefs2::{
    const_ram_storage,
};
use crypto_service::types::{LfsResult, LfsStorage};
use crypto_service::store;
use ctap_types::consts;
use fido_authenticator::SilentAuthenticator;
// use usbd_ctaphid::insecure::InsecureRamAuthenticator;

pub type FlashStorage = hal::drivers::FlashGordon;

pub type Authenticator = fido_authenticator::Authenticator<'static, CryptoSyscall, SilentAuthenticator>;

#[derive(Default)]
pub struct CryptoSyscall {}

impl crypto_service::pipe::Syscall for CryptoSyscall {
    fn syscall(&mut self) {
        rtfm::pend(hal::raw::Interrupt::OS_EVENT);
    }
}

const_ram_storage!(InternalStorage2, 8192);
const_ram_storage!(InternalStorage, 8192);
const_ram_storage!(ExternalStorage, 8192);
const_ram_storage!(VolatileStorage, 8192);

store!(Store,
    // Internal: InternalStorage,
    Internal: FlashStorage,
    External: ExternalStorage,
    Volatile: VolatileStorage
);

pub type CryptoService = crypto_service::Service<
    'static,
    hal::peripherals::rng::Rng<hal::Enabled>,
    Store,
>;

// pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, InsecureRamAuthenticator, UsbBus>;
pub type CtapHidClass = usbd_ctaphid::CtapHid<'static, 'static, UsbBus>;

pub type SerialClass = usbd_serial::SerialPort<'static, UsbBus>;
// pub type SerialClass = usbd_serial::CdcAcmClass<'static, UsbBus>;
pub type Usbd = usb_device::device::UsbDevice<'static, UsbBus>;

