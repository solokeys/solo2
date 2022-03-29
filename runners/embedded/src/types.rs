include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

pub use apdu_dispatch::{App as ApduApp, command::SIZE as ApduCommandSize, response::SIZE as ApduResponseSize};
use core::convert::TryInto;
use crate::soc::types::Soc as SocT;
pub use ctaphid_dispatch::app::{App as CtaphidApp};
use interchange::Interchange;
use littlefs2::{const_ram_storage, fs::Allocation, fs::Filesystem};
use trussed::types::{LfsResult, LfsStorage};
use trussed::{platform, store};

pub mod usbnfc;

#[derive(Clone,Copy)]
pub struct IrqNr {
	pub i: u16
}
unsafe impl cortex_m::interrupt::InterruptNumber for IrqNr {
	fn number(self) -> u16 { self.i }
}

pub const USB_ID_VENDOR_NITROKEY: u16 = 0x20a0_u16;

pub struct Config {
	pub card_issuer: &'static [u8; 13],
	pub usb_product: &'static str,
	pub usb_manufacturer: &'static str,
	pub usb_serial: &'static str,
	// pub usb_release: u16 --> taken from build_constants::USB_RELEASE
	pub usb_id_vendor: u16,
	pub usb_id_product: u16,
}

pub trait Soc {
	type InternalFlashStorage;
	type ExternalFlashStorage;
	// VolatileStorage is always RAM
	type UsbBus;
	type NfcDevice;
	type Rng;
	type TrussedUI;
	type Reboot;

	// cannot use dyn cortex_m::interrupt::Nr
	// cannot use actual types, those are usually Enums exported by the soc PAC
	const SYSCALL_IRQ: IrqNr;

	const SOC_NAME: &'static str;
	const BOARD_NAME: &'static str;
	const INTERFACE_CONFIG: &'static Config;

	fn device_uuid() -> &'static [u8; 16];
}

// 8KB of RAM
const_ram_storage!(VolatileStorage, 8192);

store!(RunnerStore,
	Internal: <SocT as Soc>::InternalFlashStorage,
	External: <SocT as Soc>::ExternalFlashStorage,
	Volatile: VolatileStorage
);

pub static mut INTERNAL_STORAGE: Option<<SocT as Soc>::InternalFlashStorage> = None;
pub static mut INTERNAL_FS_ALLOC: Option<Allocation<<SocT as Soc>::InternalFlashStorage>> = None;
pub static mut INTERNAL_FS: Option<Filesystem<<SocT as Soc>::InternalFlashStorage>> = None;
pub static mut EXTERNAL_STORAGE: Option<<SocT as Soc>::ExternalFlashStorage> = None;
pub static mut EXTERNAL_FS_ALLOC: Option<Allocation<<SocT as Soc>::ExternalFlashStorage>> = None;
pub static mut EXTERNAL_FS: Option<Filesystem<<SocT as Soc>::ExternalFlashStorage>> = None;
pub static mut VOLATILE_STORAGE: Option<VolatileStorage> = None;
pub static mut VOLATILE_FS_ALLOC: Option<Allocation<VolatileStorage>> = None;
pub static mut VOLATILE_FS: Option<Filesystem<VolatileStorage>> = None;

platform!(RunnerPlatform,
	R: <SocT as Soc>::Rng,
	S: RunnerStore,
	UI: <SocT as Soc>::TrussedUI,
);

#[derive(Default)]
pub struct RunnerSyscall {}

impl trussed::client::Syscall for RunnerSyscall {
    #[inline]
    fn syscall(&mut self) {
        rtic::pend(<SocT as Soc>::SYSCALL_IRQ);
    }
}

pub type Trussed = trussed::Service<RunnerPlatform>;
pub type TrussedClient = trussed::ClientImplementation<RunnerSyscall>;

pub type Iso14443 = nfc_device::Iso14443<<SocT as Soc>::NfcDevice>;

pub type ApduDispatch = apdu_dispatch::dispatch::ApduDispatch;
pub type CtaphidDispatch = ctaphid_dispatch::dispatch::Dispatch;

#[cfg(feature = "admin-app")]
pub type AdminApp = admin_app::App<TrussedClient, <SocT as Soc>::Reboot>;
#[cfg(feature = "piv-authenticator")]
pub type PivApp = piv_authenticator::Authenticator<TrussedClient, {ApduCommandSize}>;
#[cfg(feature = "oath-authenticator")]
pub type OathApp = oath_authenticator::Authenticator<TrussedClient>;
#[cfg(feature = "fido-authenticator")]
pub type FidoApp = fido_authenticator::Authenticator<fido_authenticator::Conforming, TrussedClient>;
#[cfg(feature = "ndef-app")]
pub type NdefApp = ndef_app::App<'static>;
#[cfg(feature = "provisioner-app")]
pub type ProvisionerApp = provisioner_app::Provisioner<RunnerStore, <SocT as Soc>::InternalFlashStorage, TrussedClient>;

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
	buf.copy_from_slice(<SocT as Soc>::device_uuid());
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
            fido_authenticator::Conforming {},
            fido_authenticator::Config { max_msg_size: usbd_ctaphid::constants::MESSAGE_SIZE }
        );

        authnr
    }
}

pub struct ProvisionerNonPortable {
    pub store: RunnerStore,
    pub stolen_filesystem: &'static mut <SocT as Soc>::InternalFlashStorage,
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
        trussed: &mut trussed::Service<RunnerPlatform>,
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

#[derive(PartialEq)]
pub enum BootMode {
	NFCPassive,
	Full
}

pub struct DummyPinError {}
pub struct DummyPin {}
impl DummyPin {
	pub fn new() -> Self { Self {} }
}
impl embedded_hal::digital::v2::OutputPin for DummyPin {
	type Error = DummyPinError;
	fn set_low(&mut self) -> Result<(), DummyPinError> { Ok(()) }
	fn set_high(&mut self) -> Result<(), DummyPinError> { Ok(()) }
}
