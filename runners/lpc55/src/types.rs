include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));
use core::convert::TryInto;

use crate::hal;
use hal::drivers::timer;
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

pub type Iso14443 = nfc_device::Iso14443<board::nfc::NfcChip>;

pub type ExternalInterrupt = hal::Pint<hal::typestates::init_state::Enabled>;

pub type ApduDispatch = apdu_dispatch::dispatch::ApduDispatch;
pub type CtaphidDispatch = ctaphid_dispatch::dispatch::Dispatch;

pub struct Lpc55Reboot {}
impl admin_app::Reboot for Lpc55Reboot {
    fn reboot() -> ! {
        hal::raw::SCB::sys_reset()
    }
    fn reboot_to_firmware_update() -> ! {
        hal::boot_to_bootrom()
    }
    fn reboot_to_firmware_update_destructive() -> ! {
        // Erasing the first flash page & rebooting will keep processor in bootrom persistently.
        // This is however destructive, as a valid firmware will need to be flashed.
        use hal::traits::flash::WriteErase;
        let flash = unsafe { hal::peripherals::flash::Flash::steal() }.enabled(
            &mut unsafe {hal::peripherals::syscon::Syscon::steal()}
        );
        hal::drivers::flash::FlashGordon::new(flash).erase_page(0).ok();
        hal::raw::SCB::sys_reset()
    }
}

#[cfg(feature = "admin-app")]
pub type AdminApp = admin_app::App<TrussedClient, Lpc55Reboot>;
#[cfg(feature = "piv-authenticator")]
pub type PivApp = piv_authenticator::Authenticator<TrussedClient>;
#[cfg(feature = "oath-authenticator")]
pub type OathApp = oath_authenticator::Authenticator<TrussedClient>;
#[cfg(feature = "fido-authenticator")]
pub type FidoApp = dispatch_fido::Fido<fido_authenticator::NonSilentAuthenticator, TrussedClient>;
#[cfg(feature = "ndef-app")]
pub type NdefApp = ndef_app::App<'static>;
#[cfg(feature = "provisioner-app")]
pub type ProvisionerApp = provisioner_app::Provisioner<Store, FlashStorage, TrussedClient>;

use apdu_dispatch::{App as ApduApp, command::Size as CommandSize, response::Size as ResponseSize};
use ctaphid_dispatch::app::{App as CtaphidApp};

pub type DynamicClockController = board::clock_controller::DynamicClockController;
pub type NfcWaitExtender = timer::Timer<ctimer::Ctimer0<hal::typestates::init_state::Enabled>>;
pub type PerformanceTimer = timer::Timer<ctimer::Ctimer4<hal::typestates::init_state::Enabled>>;

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

#[cfg(feature = "oath-authenticator")]
impl TrussedApp for OathApp {
    const CLIENT_ID: &'static [u8] = b"oath\0";

    type NonPortable = ();
    fn with_client(trussed: TrussedClient, _: ()) -> Self {
        Self::new(trussed)
    }
}

#[cfg(feature = "piv-authenticator")]
impl TrussedApp for PivApp {
    const CLIENT_ID: &'static [u8] = b"piv\0";

    type NonPortable = ();
    fn with_client(trussed: TrussedClient, _: ()) -> Self {
        Self::new(trussed)
    }
}

#[cfg(feature = "admin-app")]
impl TrussedApp for AdminApp {
    const CLIENT_ID: &'static [u8] = b"admin\0";

    // TODO: declare uuid + version
    type NonPortable = ();
    fn with_client(trussed: TrussedClient, _: ()) -> Self {
        Self::new(trussed, hal::uuid(), build_constants::CARGO_PKG_VERSION)
    }
}

#[cfg(feature = "fido-authenticator")]
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
    #[cfg(feature = "admin-app")]
    pub admin: AdminApp,
    #[cfg(feature = "fido-authenticator")]
    pub fido: FidoApp,
    #[cfg(feature = "oath-authenticator")]
    pub oath: OathApp,
    #[cfg(feature = "ndef-app")]
    pub ndef: NdefApp,
    #[cfg(feature = "piv-authenticator")]
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
        #[cfg(feature = "admin-app")]
        let admin = AdminApp::with(trussed, ());
        #[cfg(feature = "fido-authenticator")]
        let fido = FidoApp::with(trussed, ());
        #[cfg(feature = "oath-authenticator")]
        let oath = OathApp::with(trussed, ());
        #[cfg(feature = "piv-authenticator")]
        let piv = PivApp::with(trussed, ());
        #[cfg(feature = "ndef-app")]
        let ndef = NdefApp::new();
        #[cfg(feature = "provisioner-app")]
        let provisioner = ProvisionerApp::with(trussed, provisioner);

        Self {
            #[cfg(feature = "admin-app")]
            admin,
            #[cfg(feature = "fido-authenticator")]
            fido,
            #[cfg(feature = "oath-authenticator")]
            oath,
            #[cfg(feature = "ndef-app")]
            ndef,
            #[cfg(feature = "piv-authenticator")]
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
            #[cfg(feature = "ndef-app")]
            &mut self.ndef,
            #[cfg(feature = "piv-authenticator")]
            &mut self.piv,
            #[cfg(feature = "oath-authenticator")]
            &mut self.oath,
            #[cfg(feature = "fido-authenticator")]
            &mut self.fido,
            #[cfg(feature = "admin-app")]
            &mut self.admin,
            #[cfg(feature = "provisioner-app")]
            &mut self.provisioner,
        ])
    }

    pub fn ctaphid_dispatch<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut [&mut dyn CtaphidApp ]) -> T
    {
        f(&mut [
            #[cfg(feature = "fido-authenticator")]
            &mut self.fido,
            #[cfg(feature = "admin-app")]
            &mut self.admin,
        ])
    }
}
