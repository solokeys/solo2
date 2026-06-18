include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

use crate::hal;
use hal::drivers::timer;
use hal::peripherals::ctimer;
use littlefs2::{const_ram_storage, consts};
use trussed::client::MultiplexedClient;
use trussed::interrupt::InterruptFlag;
use trussed::platform;
use trussed::store::DynFilesystem;

// Compile time assertion that build_constants::CONFIG_FILESYSTEM_BOUNDARY is 512 byte aligned.
const _FILESYSTEM_ALIGNED_CHECK: usize = ((core::mem::size_of::<
    [u8; build_constants::CONFIG_FILESYSTEM_BOUNDARY % 512],
>() == 0) as usize)
    - 1;
// Compile time check that the flashregion does NOT spill over the 631.5KB boundary.
const _FILESYSTEM_WITHIN_FLASH_CHECK: usize = ((core::mem::size_of::<
    [u8; ((build_constants::CONFIG_FILESYSTEM_BOUNDARY) <= (631 * 1024 + 512)) as usize],
>() == 1) as usize)
    - 1;

pub mod littlefs_params {
    use crate::hal;
    pub const READ_SIZE: usize = 16;
    pub const WRITE_SIZE: usize = 512;
    pub const BLOCK_SIZE: usize = 512;

    // no wear-leveling for now
    pub const BLOCK_CYCLES: isize = -1;

    #[allow(non_camel_case_types, reason = "These are type-level constants")]
    pub type CACHE_SIZE = hal::drivers::flash::U512;
    #[allow(non_camel_case_types, reason = "These are type-level constants")]
    pub type LOOKAHEAD_SIZE = hal::drivers::flash::U16;
}

#[cfg(feature = "no-encrypted-storage")]
mod littlefs2_filesystem {
    use super::*;

    pub struct PlainFilesystem {
        flash_gordon: hal::drivers::flash::FlashGordon,
    }

    impl PlainFilesystem {
        const BASE_OFFSET: usize = build_constants::CONFIG_FILESYSTEM_BOUNDARY;

        pub fn new(flash_gordon: hal::drivers::flash::FlashGordon) -> Self {
            Self { flash_gordon }
        }
    }

    impl littlefs2::driver::Storage for PlainFilesystem {
        const READ_SIZE: usize = super::littlefs_params::READ_SIZE;
        const WRITE_SIZE: usize = super::littlefs_params::WRITE_SIZE;
        const BLOCK_SIZE: usize = super::littlefs_params::BLOCK_SIZE;

        const BLOCK_COUNT: usize =
            ((631 * 1024 + 512) - build_constants::CONFIG_FILESYSTEM_BOUNDARY) / 512;
        const BLOCK_CYCLES: isize = super::littlefs_params::BLOCK_CYCLES;

        type CACHE_SIZE = super::littlefs_params::CACHE_SIZE;
        type LOOKAHEAD_SIZE = super::littlefs_params::LOOKAHEAD_SIZE;

        fn read(&mut self, off: usize, buf: &mut [u8]) -> littlefs2::io::Result<usize> {
            <hal::drivers::flash::FlashGordon as hal::traits::flash::Read<
                hal::drivers::flash::U16,
            >>::read(&self.flash_gordon, Self::BASE_OFFSET + off, buf);
            Ok(buf.len())
        }

        fn write(&mut self, off: usize, data: &[u8]) -> littlefs2::io::Result<usize> {
            let ret = <hal::drivers::flash::FlashGordon as hal::traits::flash::WriteErase<
                hal::drivers::flash::U512,
                hal::drivers::flash::U512,
            >>::write(&mut self.flash_gordon, Self::BASE_OFFSET + off, data);
            ret.map(|_| data.len())
                .map_err(|_| littlefs2::io::Error::IO)
        }

        fn erase(&mut self, off: usize, len: usize) -> littlefs2::io::Result<usize> {
            let first_page = (Self::BASE_OFFSET + off) / 512;
            let pages = len / 512;
            for i in 0..pages {
                <hal::drivers::flash::FlashGordon as hal::traits::flash::WriteErase<
                    hal::drivers::flash::U512,
                    hal::drivers::flash::U512,
                >>::erase_page(&mut self.flash_gordon, first_page + i)
                .map_err(|_| littlefs2::io::Error::IO)?;
            }
            Ok(512 * len)
        }
    }
}

#[cfg(not(feature = "no-encrypted-storage"))]
mod littlefs2_prince_filesystem {
    use super::*;

    pub struct PrinceFilesystem {
        flash_gordon: hal::drivers::flash::FlashGordon,
        prince: hal::peripherals::prince::Prince<hal::typestates::init_state::Enabled>,
    }

    impl PrinceFilesystem {
        const BASE_OFFSET: usize = build_constants::CONFIG_FILESYSTEM_BOUNDARY;

        pub fn new(
            flash_gordon: hal::drivers::flash::FlashGordon,
            prince: hal::peripherals::prince::Prince<hal::typestates::init_state::Enabled>,
        ) -> Self {
            Self {
                flash_gordon,
                prince,
            }
        }
    }

    impl littlefs2::driver::Storage for PrinceFilesystem {
        const READ_SIZE: usize = super::littlefs_params::READ_SIZE;
        const WRITE_SIZE: usize = super::littlefs_params::WRITE_SIZE;
        const BLOCK_SIZE: usize = super::littlefs_params::BLOCK_SIZE;

        const BLOCK_COUNT: usize =
            ((631 * 1024 + 512) - build_constants::CONFIG_FILESYSTEM_BOUNDARY) / 512;
        const BLOCK_CYCLES: isize = super::littlefs_params::BLOCK_CYCLES;

        type CACHE_SIZE = super::littlefs_params::CACHE_SIZE;
        type LOOKAHEAD_SIZE = super::littlefs_params::LOOKAHEAD_SIZE;

        fn read(&mut self, off: usize, buf: &mut [u8]) -> littlefs2::io::Result<usize> {
            self.prince.enable_region_2_for(|| {
                let flash: *const u8 = (Self::BASE_OFFSET + off) as *const u8;
                for (i, slot) in buf.iter_mut().enumerate() {
                    *slot = unsafe { *flash.add(i) };
                }
            });
            Ok(buf.len())
        }

        fn write(&mut self, off: usize, data: &[u8]) -> littlefs2::io::Result<usize> {
            let prince = &mut self.prince;
            let flash_gordon = &mut self.flash_gordon;
            let ret = prince.write_encrypted(|prince| {
                prince.enable_region_2_for(|| {
                    <hal::drivers::flash::FlashGordon as hal::traits::flash::WriteErase<
                        hal::drivers::flash::U512,
                        hal::drivers::flash::U512,
                    >>::write(flash_gordon, Self::BASE_OFFSET + off, data)
                })
            });
            ret.map(|_| data.len())
                .map_err(|_| littlefs2::io::Error::IO)
        }

        fn erase(&mut self, off: usize, len: usize) -> littlefs2::io::Result<usize> {
            let first_page = (Self::BASE_OFFSET + off) / 512;
            let pages = len / 512;
            for i in 0..pages {
                <hal::drivers::flash::FlashGordon as hal::traits::flash::WriteErase<
                    hal::drivers::flash::U512,
                    hal::drivers::flash::U512,
                >>::erase_page(&mut self.flash_gordon, first_page + i)
                .map_err(|_| littlefs2::io::Error::IO)?;
            }
            Ok(512 * len)
        }
    }
}

#[cfg(feature = "no-encrypted-storage")]
pub use littlefs2_filesystem::PlainFilesystem;
#[cfg(feature = "no-encrypted-storage")]
pub type FlashStorage = PlainFilesystem;
#[cfg(not(feature = "no-encrypted-storage"))]
pub use littlefs2_prince_filesystem::PrinceFilesystem;
#[cfg(not(feature = "no-encrypted-storage"))]
pub type FlashStorage = PrinceFilesystem;

pub mod usb;
pub use usb::{CcidClass, CtapHidClass, EnabledUsbPeripheral, SerialClass, UsbClasses};

// 8KB of RAM
const_ram_storage!(
    name = VolatileStorage,
    erase_value = 0xff,
    read_size = 1,
    write_size = 1,
    cache_size_ty = consts::U128,
    // this is a limitation of littlefs
    // https://git.io/JeHp9
    block_size = 128,
    // block_size=128,
    block_count = 8192 / 104,
    lookahead_size_ty = consts::U8,
    filename_max_plus_one_ty = consts::U256,
    path_max_plus_one_ty = consts::U256,
);

pub type ExternalStorage = board::flash::Solo2ExtFlash;

// On real hardware secrets-app lives on the external flash chip; this RAM
// fallback is only used when no chip is present (e.g. a bare EVK).
const_ram_storage!(ExternalFallbackStorage, 4096);

/// Store implementation using three mounted littlefs2 filesystems.
#[derive(Clone, Copy)]
pub struct RunnerStore {
    ifs: &'static dyn DynFilesystem,
    efs: &'static dyn DynFilesystem,
    vfs: &'static dyn DynFilesystem,
}

impl RunnerStore {
    pub fn new(
        ifs: &'static dyn DynFilesystem,
        efs: &'static dyn DynFilesystem,
        vfs: &'static dyn DynFilesystem,
    ) -> Self {
        Self { ifs, efs, vfs }
    }
}

impl trussed::store::Store for RunnerStore {
    fn ifs(&self) -> &dyn DynFilesystem {
        self.ifs
    }
    fn efs(&self) -> &dyn DynFilesystem {
        self.efs
    }
    fn vfs(&self) -> &dyn DynFilesystem {
        self.vfs
    }
}

pub type Store = RunnerStore;

pub type ThreeButtons = board::ThreeButtons;
pub type RgbLed = board::RgbLed;

platform!(Board,
    R: hal::peripherals::rng::Rng<hal::Enabled>,
    S: Store,
    UI: board::trussed::UserInterface<ThreeButtons, RgbLed>,
);

// Trussed extension dispatch — shared with the nrf52840dk runner via `solo-apps`.
use solo_apps::client::{client_tag, make_client};
pub use solo_apps::dispatch::{BackendIds, Dispatch, RunnerContext};
use solo_apps::ndef::{FidoNdefStamp, NdefFidoGate};

#[derive(Default)]
pub struct Syscall {}

impl trussed::client::Syscall for Syscall {
    #[inline]
    fn syscall(&mut self) {
        rtic::pend(board::hal::raw::Interrupt::OS_EVENT);
    }
}

/// Client type for apps — all apps share the channel inside the multiplexed
/// service and are distinguished by a per-app `ClientTag`.
pub type TrussedClient = MultiplexedClient<Syscall, Dispatch>;

// Backend slates, the multiplexed service wrapper (`Trussed`), `client_tag`, and
// `make_client` are shared with the nrf52840dk runner via `solo_apps::client`.
pub type Trussed = solo_apps::client::Trussed<Board>;

pub type Iso14443 = nfc_device::Iso14443<'static, board::nfc::NfcChip>;

pub type ExternalInterrupt = hal::Pint<hal::typestates::init_state::Enabled>;

pub type ApduDispatch = apdu_dispatch::dispatch::ApduDispatch<'static>;
pub type CtaphidDispatch =
    ctaphid_dispatch::Dispatch<'static, 'static, { ctaphid_dispatch::DEFAULT_MESSAGE_SIZE }>;

/// Minimal status implementation for admin-app.
#[cfg(feature = "admin-app")]
#[derive(Default)]
pub struct AdminStatus {
    random_error: bool,
}

#[cfg(feature = "admin-app")]
impl admin_app::StatusBytes for AdminStatus {
    type Serialized = [u8; 1];

    fn set_random_error(&mut self, value: bool) {
        self.random_error = value;
    }

    fn get_random_error(&self) -> bool {
        self.random_error
    }

    fn serialize(&self) -> Self::Serialized {
        [self.random_error as u8]
    }
}

#[cfg(feature = "admin-app")]
pub type AdminApp = admin_app::App<TrussedClient, board::Reboot, AdminStatus>;
#[cfg(feature = "piv-authenticator")]
pub type PivApp = piv_authenticator::Authenticator<TrussedClient>;
#[cfg(feature = "opcard")]
pub type OpcardApp = opcard::Card<TrussedClient>;
#[cfg(feature = "oath")]
pub type SecretsApp = secrets_app::Authenticator<TrussedClient>;
#[cfg(feature = "oath-export")]
pub type OathExportApp = oath_export::OathExport<Store, TrussedClient>;
#[cfg(feature = "fido-authenticator")]
pub type FidoApp = fido_authenticator::Authenticator<fido_authenticator::Conforming, TrussedClient>;
#[cfg(feature = "fido-authenticator")]
pub type FidoConfig = fido_authenticator::Config;

/// FIDO authenticator config — shared with the nrf52840dk runner. `max_resident`
/// is 100 here; the `nfc_transport` regression guard lives in `solo-apps`.
pub const FIDO_CONFIG: FidoConfig =
    solo_apps::config::fido_config(build_constants::CARGO_PKG_VERSION, 100);
#[cfg(feature = "ndef-app")]
pub type NdefApp = ndef_app::App<TrussedClient>;

/// NDEF suppression. The NDEF app refuses `SELECT` (so phones don't pop the tag
/// during/after a FIDO ceremony) while we're within `NDEF_SUPPRESS_SECS` of the last
/// FIDO command. The timestamp is the always-on 32kHz RTC `COUNT` (1Hz) — independent
/// of the dynamic CPU clock, so it is a valid timebase in passive too, unlike a
/// SysTick/Mono counter. `NDEF_LAST_FIDO_SEC` starts `NDEF_SUPPRESS_SECS` "in the
/// past" so the gap is already `>= NDEF_SUPPRESS_SECS` at boot (NOT suppressed) — the
/// tag stays readable until an actual FIDO arms the window (the OS does FIDO before
/// NDEF anyway); a fresh boot (field drop between taps) starts readable again.
const NDEF_SUPPRESS_SECS: u32 = 7;
pub static NDEF_LAST_FIDO_SEC: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0u32.wrapping_sub(NDEF_SUPPRESS_SECS));

/// Seconds from the always-on RTC `COUNT` register, read directly (the RTC handle
/// lives in the UserInterface, out of reach of these apps).
fn rtc_secs() -> u32 {
    // SAFETY: read-only access to a free-running hardware counter; races are benign.
    unsafe { (*hal::raw::RTC::ptr()).count.read().bits() }
}

/// lpc55 NDEF timebase + FIDO-transport hook for `solo_apps::ndef`. `now()` is
/// the always-on RTC COUNT (1 Hz, valid in passive); `set_fido_over_nfc` records
/// the transport so `check_user_presence` takes an NFC tap as presence.
pub struct LpcNfcClock;
impl solo_apps::ndef::NfcClock for LpcNfcClock {
    const SUPPRESS_WINDOW: u32 = NDEF_SUPPRESS_SECS;
    fn now() -> u32 {
        rtc_secs()
    }
    fn last_fido() -> &'static core::sync::atomic::AtomicU32 {
        &NDEF_LAST_FIDO_SEC
    }
    fn set_fido_over_nfc(contactless: bool) {
        board::trussed::FIDO_OVER_NFC.store(contactless, core::sync::atomic::Ordering::Relaxed);
    }
}
#[cfg(feature = "provisioner-app")]
pub type ProvisionerApp = provisioner_app::Provisioner<Store, FlashStorage, TrussedClient>;

use apdu_dispatch::App as ApduApp;
use ctaphid_dispatch::app::App as CtaphidApp;

pub type DynamicClockController = board::clock_controller::DynamicClockController;
pub type NfcWaitExtender = timer::Timer<ctimer::Ctimer0<hal::typestates::init_state::Enabled>>;
pub type PerformanceTimer = timer::Timer<ctimer::Ctimer4<hal::typestates::init_state::Enabled>>;

#[cfg(feature = "admin-app")]
static ADMIN_INTERRUPT: InterruptFlag = InterruptFlag::new();
#[cfg(feature = "ndef-app")]
static NDEF_INTERRUPT: InterruptFlag = InterruptFlag::new();
#[cfg(feature = "fido-authenticator")]
static FIDO_INTERRUPT: InterruptFlag = InterruptFlag::new();
#[cfg(feature = "piv-authenticator")]
static PIV_INTERRUPT: InterruptFlag = InterruptFlag::new();
#[cfg(feature = "opcard")]
static OPCARD_INTERRUPT: InterruptFlag = InterruptFlag::new();
#[cfg(feature = "provisioner-app")]
static PROVISIONER_INTERRUPT: InterruptFlag = InterruptFlag::new();
#[cfg(feature = "oath")]
static SECRETS_INTERRUPT: InterruptFlag = InterruptFlag::new();
#[cfg(feature = "oath-export")]
static OATH_EXPORT_INTERRUPT: InterruptFlag = InterruptFlag::new();

pub struct ProvisionerNonPortable {
    pub store: Store,
    pub stolen_filesystem: &'static mut FlashStorage,
    pub nfc_powered: bool,
}

pub struct Apps {
    #[cfg(feature = "admin-app")]
    pub admin: AdminApp,
    #[cfg(feature = "fido-authenticator")]
    pub fido: FidoApp,
    #[cfg(feature = "oath")]
    pub secrets: SecretsApp,
    #[cfg(feature = "ndef-app")]
    pub ndef: NdefApp,
    #[cfg(feature = "piv-authenticator")]
    pub piv: PivApp,
    #[cfg(feature = "opcard")]
    pub opcard: OpcardApp,
    #[cfg(feature = "provisioner-app")]
    pub provisioner: ProvisionerApp,
    #[cfg(feature = "oath-export")]
    pub oath_export: OathExportApp,
}

impl Apps {
    pub fn new(
        trussed: &mut Trussed,
        #[cfg(feature = "provisioner-app")] provisioner_np: ProvisionerNonPortable,
        #[cfg(feature = "oath-export")] store: Store,
    ) -> Self {
        #[cfg(feature = "admin-app")]
        let admin = {
            let client = make_client(
                client_tag::ADMIN,
                littlefs2::path!("admin"),
                trussed,
                Some(&ADMIN_INTERRUPT),
                &solo_apps::client::STAGING_BACKENDS,
            );
            AdminApp::with_default_config(
                client,
                hal::uuid(),
                build_constants::CARGO_PKG_VERSION,
                env!("CARGO_PKG_VERSION"),
                AdminStatus::default(),
                &[],
            )
        };

        #[cfg(feature = "fido-authenticator")]
        let fido = {
            let client = make_client(
                client_tag::FIDO,
                littlefs2::path!("fido"),
                trussed,
                Some(&FIDO_INTERRUPT),
                &solo_apps::client::STAGING_BACKENDS,
            );
            fido_authenticator::Authenticator::new(
                client,
                fido_authenticator::Conforming {},
                FIDO_CONFIG,
            )
        };

        #[cfg(feature = "piv-authenticator")]
        let piv = {
            let client = make_client(
                client_tag::PIV,
                littlefs2::path!("piv"),
                trussed,
                Some(&PIV_INTERRUPT),
                &solo_apps::client::PIV_BACKENDS,
            );
            PivApp::new(
                client,
                piv_authenticator::Options::default().storage(trussed::types::Location::External),
            )
        };

        #[cfg(feature = "opcard")]
        let opcard = {
            let client = make_client(
                client_tag::OPCARD,
                littlefs2::path!("opcard"),
                trussed,
                Some(&OPCARD_INTERRUPT),
                &solo_apps::client::PIV_BACKENDS,
            );
            {
                let mut opts = opcard::Options::default();
                opts.storage = trussed::types::Location::External;
                OpcardApp::new(client, opts)
            }
        };

        #[cfg(feature = "oath")]
        let secrets = {
            let client = make_client(
                client_tag::SECRETS,
                littlefs2::path!("secrets"),
                trussed,
                Some(&SECRETS_INTERRUPT),
                &solo_apps::client::AUTH_BACKENDS,
            );
            let uuid = hal::uuid();
            SecretsApp::new(
                client,
                secrets_app::Options::new(
                    trussed::types::Location::External,
                    0, // custom_status_reverse_hotp_success
                    1, // custom_status_reverse_hotp_error
                    [uuid[0], uuid[1], uuid[2], uuid[3]],
                    100, // max_resident_credentials_allowed
                ),
            )
        };

        #[cfg(feature = "ndef-app")]
        let ndef = {
            let client = make_client(
                client_tag::NDEF,
                littlefs2::path!("ndef"),
                trussed,
                Some(&NDEF_INTERRUPT),
                &solo_apps::client::STAGING_BACKENDS,
            );
            NdefApp::new(client)
        };

        #[cfg(feature = "provisioner-app")]
        let provisioner = {
            let client = make_client(
                client_tag::PROVISIONER,
                littlefs2::path!("attn"),
                trussed,
                Some(&PROVISIONER_INTERRUPT),
                &solo_apps::client::STAGING_BACKENDS,
            );
            let ProvisionerNonPortable {
                store,
                stolen_filesystem,
                nfc_powered,
            } = provisioner_np;
            ProvisionerApp::new(client, store, stolen_filesystem, nfc_powered)
        };

        #[cfg(feature = "oath-export")]
        let oath_export = {
            let client = make_client(
                client_tag::OATH_EXPORT,
                littlefs2::path!("oathmig"),
                trussed,
                Some(&OATH_EXPORT_INTERRUPT),
                &solo_apps::client::STAGING_BACKENDS,
            );
            OathExportApp::new(client, store)
        };

        Self {
            #[cfg(feature = "admin-app")]
            admin,
            #[cfg(feature = "fido-authenticator")]
            fido,
            #[cfg(feature = "oath")]
            secrets,
            #[cfg(feature = "ndef-app")]
            ndef,
            #[cfg(feature = "piv-authenticator")]
            piv,
            #[cfg(feature = "opcard")]
            opcard,
            #[cfg(feature = "provisioner-app")]
            provisioner,
            #[cfg(feature = "oath-export")]
            oath_export,
        }
    }

    #[inline(never)]
    pub fn apdu_dispatch<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut [&mut dyn ApduApp]) -> T,
    {
        f(&mut [
            #[cfg(feature = "ndef-app")]
            &mut NdefFidoGate::<LpcNfcClock, _>::new(&mut self.ndef),
            #[cfg(feature = "piv-authenticator")]
            &mut self.piv,
            #[cfg(feature = "opcard")]
            &mut self.opcard,
            #[cfg(feature = "oath")]
            &mut self.secrets,
            #[cfg(feature = "fido-authenticator")]
            &mut FidoNdefStamp::<LpcNfcClock, _>::new(&mut self.fido),
            #[cfg(feature = "admin-app")]
            &mut self.admin,
            #[cfg(feature = "provisioner-app")]
            &mut self.provisioner,
            #[cfg(feature = "oath-export")]
            &mut self.oath_export,
        ])
    }

    #[inline(never)]
    pub fn ctaphid_dispatch<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut [&mut dyn CtaphidApp<'static>]) -> T,
    {
        // USB transport: a CTAPHID request is never NFC, so user presence
        // requires a button.
        board::trussed::FIDO_OVER_NFC.store(false, core::sync::atomic::Ordering::Relaxed);
        f(&mut [
            #[cfg(feature = "admin-app")]
            &mut self.admin,
            #[cfg(feature = "fido-authenticator")]
            &mut self.fido,
            #[cfg(feature = "oath")]
            &mut self.secrets,
        ])
    }
}
