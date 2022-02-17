include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

use apdu_dispatch::{App as ApduApp, command::SIZE as ApduCommandSize, response::SIZE as ApduResponseSize};
use core::convert::TryInto;
use crate::soc;
use ctaphid_dispatch::app::{App as CtaphidApp};
use interchange::Interchange;
use littlefs2::{const_ram_storage, consts};
use trussed::types::{LfsResult, LfsStorage};
use trussed::{platform, store};

pub mod usb;
pub use usb::{UsbClasses, SerialClass, CcidClass, CtapHidClass};

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

store!(RunnerStore,
	Internal: soc::types::FlashStorage,
	External: soc::types::ExternalStorage,
	Volatile: VolatileStorage
);

platform!(RunnerBoard,
	R: soc::types::Rng,
	S: RunnerStore,
	UI: soc::types::TrussedUI,
);

#[derive(Default)]
pub struct RunnerSyscall {}

impl trussed::client::Syscall for RunnerSyscall {
    #[inline]
    fn syscall(&mut self) {
        rtic::pend(soc::types::SYSCALL_IRQ);
    }
}

pub type Trussed = trussed::Service<RunnerBoard>;
pub type TrussedClient = trussed::ClientImplementation<RunnerSyscall>;

// pub type Iso14443 = nfc_device::Iso14443<board::soc::nfc::NfcChip>;
pub struct Iso14443 {}

pub type ApduDispatch = apdu_dispatch::dispatch::ApduDispatch;
pub type CtaphidDispatch = ctaphid_dispatch::dispatch::Dispatch;

#[cfg(feature = "admin-app")]
pub type AdminApp = admin_app::App<TrussedClient, soc::types::Reboot>;
#[cfg(feature = "piv-authenticator")]
pub type PivApp = piv_authenticator::Authenticator<TrussedClient, {ApduCommandSize}>;
#[cfg(feature = "oath-authenticator")]
pub type OathApp = oath_authenticator::Authenticator<TrussedClient>;
#[cfg(feature = "fido-authenticator")]
pub type FidoApp = dispatch_fido::Fido<fido_authenticator::NonSilentAuthenticator, TrussedClient>;
#[cfg(feature = "ndef-app")]
pub type NdefApp = ndef_app::App<'static>;
#[cfg(feature = "provisioner-app")]
pub type ProvisionerApp = provisioner_app::Provisioner<RunnerStore, soc::types::FlashStorage, TrussedClient>;

pub trait TrussedApp: Sized {

    /// non-portable resources needed by this Trussed app
    type NonPortable;

    /// the desired client ID
    const CLIENT_ID: &'static [u8];

    fn with_client(trussed: TrussedClient, non_portable: Self::NonPortable) -> Self;

    fn with(trussed: &mut Trussed, non_portable: Self::NonPortable) -> Self {
        let (trussed_requester, trussed_responder) = trussed::pipe::TrussedInterchange::claim()
            .expect("could not setup TrussedInterchange");

        let mut client_id = littlefs2::path::PathBuf::new();
        client_id.push(Self::CLIENT_ID.try_into().unwrap());
        assert!(trussed.add_endpoint(trussed_responder, client_id).is_ok());

        let syscaller = RunnerSyscall::default();
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
        let mut buf: [u8; 16] = [0u8; 16];
	buf.copy_from_slice(soc::types::device_uuid());
        Self::new(trussed, buf, build_constants::CARGO_PKG_VERSION)
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
    pub store: RunnerStore,
    pub stolen_filesystem: &'static mut soc::types::FlashStorage,
    pub nfc_powered: bool,
}

#[cfg(feature = "provisioner-app")]
impl TrussedApp for ProvisionerApp {
    const CLIENT_ID: &'static [u8] = b"attn\0";

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
        trussed: &mut trussed::Service<RunnerBoard>,
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
                ApduApp<ApduCommandSize, ApduResponseSize>
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

#[derive(Debug)]
pub struct DelogFlusher {}

impl delog::Flusher for DelogFlusher {
	fn flush(&self, _msg: &str) {
		#[cfg(feature = "log-rtt")]
		rtt_target::rprint!(_msg);

		#[cfg(feature = "log-semihosting")]
		cortex_m_semihosting::hprint!(_msg).ok();
	}
}

pub static DELOG_FLUSHER: DelogFlusher = DelogFlusher {};
