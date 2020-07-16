use crate::hal;
use hal::drivers::{pins,pins::Pin,SpiMaster, timer};
use littlefs2::{
    const_ram_storage,
};
use trussed::types::{LfsResult, LfsStorage};
use trussed::store;
use ctap_types::consts;
use fido_authenticator::SilentAuthenticator;
use fm11nc08::FM11NC08;
use hal::{
    typestates::{
        pin::flexcomm::{
            NoPio,
        },
        pin::{
            state,
            function,
            gpio::{
                direction,
            }
        }
    },
    peripherals::ctimer,
};
// use usbd_ctaphid::insecure::InsecureRamAuthenticator;

pub type FlashStorage = hal::drivers::FlashGordon;

pub type Authenticator = fido_authenticator::Authenticator<SilentAuthenticator>;

pub type Piv = piv_card::App;

pub use trussed::client::TrussedSyscall;

pub mod usb;
pub use usb::{UsbClasses, EnabledUsbPeripheral, SerialClass, CcidClass, CtapHidClass};

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

pub type CryptoService = trussed::Service<
    hal::peripherals::rng::Rng<hal::Enabled>,
    Store,
>;

pub type NfcSckPin = pins::Pio0_28;
pub type NfcMosiPin = pins::Pio0_24;
pub type NfcMisoPin = pins::Pio0_25;
pub type NfcCsPin = pins::Pio1_20;
pub type NfcIrqPin = pins::Pio0_19;

pub type NfcChip = FM11NC08<
            SpiMaster<
                NfcSckPin,
                NfcMosiPin,
                NfcMisoPin,
                NoPio,
                hal::peripherals::flexcomm::Spi0,
                (
                    Pin<NfcSckPin, state::Special<function::FC0_SCK>>,
                    Pin<NfcMosiPin, state::Special<function::FC0_RXD_SDA_MOSI_DATA>>,
                    Pin<NfcMisoPin, state::Special<function::FC0_TXD_SCL_MISO_WS>>,
                    hal::typestates::pin::flexcomm::NoCs,
                )
                >,
                Pin<NfcCsPin, state::Gpio<direction::Output>>,
                Pin<NfcIrqPin, state::Gpio<direction::Input>>,
            >;
pub type Iso14443 = iso14443::Iso14443<NfcChip>;

pub type ExternalInterrupt = hal::Pint<hal::typestates::init_state::Enabled>;

pub type ApduDispatch = apdu_dispatch::dispatch::ApduDispatch;

pub type FidoApplet = applet_fido::Fido;

pub type PerfTimer = timer::Timer<ctimer::Ctimer4<hal::typestates::init_state::Enabled>>;

pub type DynamicClockController = crate::clock_controller::DynamicClockController;

pub type SignalPin = pins::Pio0_23;
pub type SignalButton = Pin<SignalPin, state::Gpio<direction::Output>>;

pub type HwScheduler = timer::Timer<ctimer::Ctimer0<hal::typestates::init_state::Enabled>>;
