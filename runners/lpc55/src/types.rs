include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));
use core::convert::TryInto;

use crate::hal;
use hal::drivers::{pins, timer};
use interchange::Interchange;
use littlefs2::const_ram_storage;
use trussed::types::{LfsResult, LfsStorage};
use trussed::{platform, store};
use ctap_types::consts;
use hal::peripherals::ctimer;

#[cfg(feature = "no-encrypted-storage")]
use hal::littlefs2_filesystem;
#[cfg(not(feature = "no-encrypted-storage"))]
use hal::littlefs2_prince_filesystem;

#[cfg(feature = "no-encrypted-storage")]
littlefs2_filesystem!(PlainFilesystem: (build_constants::CONFIG_FILESYSTEM_BOUNDARY));
#[cfg(not(feature = "no-encrypted-storage"))]
littlefs2_prince_filesystem!(PrinceFilesystem: (build_constants::CONFIG_FILESYSTEM_BOUNDARY));

#[cfg(feature = "no-encrypted-storage")]
pub type FlashStorage = PlainFilesystem;
#[cfg(not(feature = "no-encrypted-storage"))]
pub type FlashStorage = PrinceFilesystem;

pub mod usb;
pub use usb::{UsbClasses, EnabledUsbPeripheral, SerialClass, CcidClass, CtapHidClass};

// 8KB of RAM
const_ram_storage!(
    name=VolatileStorage,
    trait=LfsStorage,
    erase_value=0xff,
    read_size=1,
    write_size=1,
    cache_size_ty=consts::U128,
    // this is a limitation of littlefs
    // https://git.io/JeHp9
    block_size=128,
    // block_size=128,
    block_count=8192/104,
    lookaheadwords_size_ty=consts::U8,
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

pub type ThreeButtons = board::ThreeButtons;
pub type RgbLed = board::RgbLed;

platform!(Board,
    R: hal::peripherals::rng::Rng<hal::Enabled>,
    S: Store,
    UI: board::trussed::UserInterface<ThreeButtons, RgbLed>,
);

#[derive(Default)]
pub struct Syscall {}

impl trussed::client::Syscall for Syscall {
    #[inline]
    fn syscall(&mut self) {
        rtic::pend(board::hal::raw::Interrupt::OS_EVENT);
    }
}

pub type Trussed = trussed::Service<Board>;
pub type TrussedClient = trussed::ClientImplementation<Syscall>;

pub type NfcSckPin = pins::Pio0_28;
pub type NfcMosiPin = pins::Pio0_24;
pub type NfcMisoPin = pins::Pio0_25;
pub type NfcCsPin = pins::Pio1_20;
pub type NfcIrqPin = pins::Pio0_19;

// pub use board::NfcChip;
pub type Iso14443 = nfc_device::Iso14443<board::nfc::NfcChip>;

pub type ExternalInterrupt = hal::Pint<hal::typestates::init_state::Enabled>;

pub type ApduDispatch = apdu_dispatch::dispatch::ApduDispatch;
pub type CtaphidDispach = ctaphid_dispatch::dispatch::Dispatch;

pub type PivApp = piv_authenticator::Authenticator<TrussedClient>;
pub type OathApp = oath_authenticator::Authenticator<TrussedClient>;
pub type FidoApp = dispatch_fido::Fido<fido_authenticator::NonSilentAuthenticator, TrussedClient>;
pub type ManagementApp = management_app::App<TrussedClient>;
pub type NdefApp = ndef_app::App<'static>;
#[cfg(feature = "provisioner-app")]
pub type ProvisionerApp = provisioner_app::Provisioner<Store, FlashStorage, TrussedClient>;

use apdu_dispatch::{App as ApduApp, command::Size as CommandSize, response::Size as ResponseSize};
use ctaphid_dispatch::app::{App as CtaphidApp};

pub type PerfTimer = timer::Timer<ctimer::Ctimer4<hal::typestates::init_state::Enabled>>;
pub type DynamicClockController = board::clock_controller::DynamicClockController;
pub type HwScheduler = timer::Timer<ctimer::Ctimer0<hal::typestates::init_state::Enabled>>;

pub trait TrussedApp: Sized {

    /// non-portable resources needed by this Trussed app
    type NonPortable;

    /// the desired client ID
    const CLIENT_ID: &'static [u8];

    fn with_client(trussed: TrussedClient, non_portable: Self::NonPortable) -> Self;

    fn with(trussed: &mut trussed::Service<crate::Board>, non_portable: Self::NonPortable) -> Self {
        let (trussed_requester, trussed_responder) = trussed::pipe::TrussedInterchange::claim()
            .expect("could not setup TrussedInterchange");

        let mut client_id = littlefs2::path::PathBuf::new();
        client_id.push(Self::CLIENT_ID.try_into().unwrap());
        assert!(trussed.add_endpoint(trussed_responder, client_id).is_ok());

        let syscaller = Syscall::default();
        let trussed_client = TrussedClient::new(
            trussed_requester,
            syscaller,
        );

        let app = Self::with_client(trussed_client, non_portable);
        app
    }
}

impl TrussedApp for OathApp {
    const CLIENT_ID: &'static [u8] = b"oath\0";

    type NonPortable = ();
    fn with_client(trussed: TrussedClient, _: ()) -> Self {
        Self::new(trussed)
    }
}

impl TrussedApp for PivApp {
    const CLIENT_ID: &'static [u8] = b"piv\0";

    type NonPortable = ();
    fn with_client(trussed: TrussedClient, _: ()) -> Self {
        Self::new(trussed)
    }
}

impl TrussedApp for ManagementApp {
    const CLIENT_ID: &'static [u8] = b"mgmt\0";

    // TODO: declare uuid + version
    type NonPortable = ();
    fn with_client(trussed: TrussedClient, _: ()) -> Self {
        Self::new(trussed, hal::uuid(), build_constants::CARGO_PKG_VERSION)
    }
}

impl TrussedApp for FidoApp {
    const CLIENT_ID: &'static [u8] = b"fido\0";

    type NonPortable = ();
    fn with_client(trussed: TrussedClient, _: ()) -> Self {
        let authnr = fido_authenticator::Authenticator::new(
            trussed,
            fido_authenticator::NonSilentAuthenticator {},
        );

        Self::new(authnr)
    }
}

pub struct ProvisionerNonPortable {
    pub store: Store,
    pub stolen_filesystem: &'static mut FlashStorage,
    pub nfc_powered: bool,
}

#[cfg(feature = "provisioner-app")]
impl TrussedApp for ProvisionerApp {
    const CLIENT_ID: &'static [u8] = b"pro\0";

    type NonPortable = ProvisionerNonPortable;
    fn with_client(trussed: TrussedClient, ProvisionerNonPortable { store, stolen_filesystem, nfc_powered }: Self::NonPortable) -> Self {
        Self::new(trussed, store, stolen_filesystem, nfc_powered)
    }

}

pub struct Apps {
    pub mgmt: ManagementApp,
    pub fido: FidoApp,
    pub oath: OathApp,
    pub ndef: NdefApp,
    pub piv: PivApp,
    #[cfg(feature = "provisioner-app")]
    pub provisioner: ProvisionerApp,
}

impl Apps {
    pub fn new(
        trussed: &mut trussed::Service<crate::Board>,
        #[cfg(feature = "provisioner-app")]
        provisioner: ProvisionerNonPortable
    ) -> Self {
        let mgmt = ManagementApp::with(trussed, ());
        let fido = FidoApp::with(trussed, ());
        let oath = OathApp::with(trussed, ());
        let piv = PivApp::with(trussed, ());
        let ndef = NdefApp::new();
        #[cfg(feature = "provisioner-app")]
        let provisioner = ProvisionerApp::with(trussed, provisioner);

        Self {
            mgmt,
            fido,
            oath,
            ndef,
            piv,
            #[cfg(feature = "provisioner-app")]
            provisioner,
        }
    }

    pub fn apdu_dispatch<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut [&mut dyn
                ApduApp<CommandSize, ResponseSize>
            ]) -> T
    {
        f(&mut [
            &mut self.ndef,
            &mut self.piv,
            &mut self.oath,
            &mut self.fido,
            &mut self.mgmt,
            #[cfg(feature = "provisioner-app")]
            &mut self.provisioner,
        ])
    }

    pub fn ctaphid_dispatch<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut [&mut dyn CtaphidApp ]) -> T
    {
        f(&mut [
            &mut self.fido,
            &mut self.mgmt,
        ])
    }
}
