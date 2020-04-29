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

// 8KB of RAM
const_ram_storage!(
    name=VolatileStorage,
    trait=LfsStorage,
    erase_value=0xff,
    read_size=1,
    write_size=1,
    cache_size_ty=consts::U104,
    // this is a limitation of littlefs
    // https://git.io/JeHp9
    block_size=104,
    // block_size=128,
    block_count=8192/104,
    lookaheadwords_size_ty=consts::U1,
    filename_max_plus_one_ty=consts::U256,
    path_max_plus_one_ty=consts::U256,
    result=LfsResult,
);

// minimum: 2 blocks
// TODO: make this optional
const_ram_storage!(ExternalStorage, 1024);

store!(Store,
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

