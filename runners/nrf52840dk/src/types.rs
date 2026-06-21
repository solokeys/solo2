//! Trussed platform + apps wiring for the nRF52840-DK runner.
//!
//! Default app set: admin, fido, ndef, secrets, piv, opcard.

/// Pack `CARGO_PKG_VERSION_*` into the same u32 layout the LPC55 runner uses
/// for `firmware_version` (CTAP 2.1 §6.4 0x0E): `(major << 22) | (minor << 6)
/// | patch`. Major < 1024, minor < 16384, patch < 64; built from the package
/// version at compile time.
const fn pkg_version_u32() -> u32 {
    const fn parse(s: &str) -> u32 {
        let bytes = s.as_bytes();
        let mut acc = 0u32;
        let mut i = 0;
        while i < bytes.len() {
            acc = acc * 10 + (bytes[i] - b'0') as u32;
            i += 1;
        }
        acc
    }
    let major = parse(env!("CARGO_PKG_VERSION_MAJOR"));
    let minor = parse(env!("CARGO_PKG_VERSION_MINOR"));
    let patch = parse(env!("CARGO_PKG_VERSION_PATCH"));
    (major << 22) | (minor << 6) | patch
}

use cortex_m::peripheral::SCB;
use generic_array::typenum::{U128, U8};
use littlefs2::const_ram_storage;
use nrf52840_hal::rng::Rng;
use trussed::client::MultiplexedClient;
use trussed::interrupt::InterruptFlag;
use trussed::platform;
use trussed::store::DynFilesystem;

use crate::board::{Syscall, UserInterface};
use crate::dispatch::Dispatch;
use solo_apps::client::{client_tag, make_client};
use solo_apps::ndef::{FidoNdefStamp, NdefFidoGate};

// ───── Reboot impl required by admin-app ─────────────────────────────────────

pub struct Reboot;
impl admin_app::Reboot for Reboot {
    fn reboot() -> ! {
        SCB::sys_reset()
    }
    /// Reboot into the Nordic Open Bootloader's DFU mode.
    /// `GPREGRET = 0xB1` is the SDK-defined BOOTLOADER_DFU_START sentinel —
    /// the bootloader checks this byte first thing on reset and enters DFU
    /// (USB CDC) mode if it sees it. The register is in retained RAM, so it
    /// survives the soft reset.
    fn reboot_to_firmware_update() -> ! {
        const BOOTLOADER_DFU_START: u8 = 0xB1;
        unsafe {
            (*nrf52840_pac::POWER::PTR)
                .gpregret
                .write(|w| w.gpregret().bits(BOOTLOADER_DFU_START));
        }
        SCB::sys_reset()
    }
    fn reboot_to_firmware_update_destructive() -> ! {
        Self::reboot_to_firmware_update()
    }
    fn locked() -> bool {
        false
    }
}

// ───── Storage ───────────────────────────────────────────────────────────────

const_ram_storage!(
    name = VolatileStorage,
    erase_value = 0xff,
    read_size = 1,
    write_size = 1,
    cache_size_ty = U128,
    block_size = 128,
    block_count = 8192 / 104,
    lookahead_size_ty = U8,
    filename_max_plus_one_ty = generic_array::typenum::U256,
    path_max_plus_one_ty = generic_array::typenum::U256,
);

// External storage is aliased to the same NVMC-backed filesystem as
// `ifs` in main.rs — see the RunnerStore construction. piv-authenticator
// and opcard hardcode `Location::External` for a couple of small backup
// paths (PUK / admin-key); routing them at the same flash filesystem
// makes them persist without a separate backing.

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

// ───── Platform ──────────────────────────────────────────────────────────────

platform!(
    Board,
    R: Rng,
    S: Store,
    UI: UserInterface,
);

// ───── Trussed Service wrapper + Backend slates ──────────────────────────────

/// All apps share the channel inside the multiplexed service and are
/// distinguished by a per-app `ClientTag`.
pub type TrussedClient = MultiplexedClient<Syscall, Dispatch>;

/// The multiplexed service wrapper (`Trussed`), backend slates, `client_tag`,
/// and `make_client` are shared with the lpc55 runner via `solo_apps::client`.
pub type Trussed = solo_apps::client::Trussed<Board>;

// ───── App types ─────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct AdminStatus {
    random_error: bool,
}

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

pub type AdminApp =
    admin_app::App<TrussedClient, Reboot, AdminStatus, crate::device_config::DeviceConfig>;
pub type FidoApp = fido_authenticator::Authenticator<fido_authenticator::Conforming, TrussedClient>;
pub type NdefApp = ndef_app::App<TrussedClient>;
pub type SecretsApp = secrets_app::Authenticator<TrussedClient>;
pub type PivApp = piv_authenticator::Authenticator<TrussedClient>;
pub type OpcardApp = opcard::Card<TrussedClient>;

// ── Wallet app (kept OUTSIDE `Apps`; its sign blocks on user presence) ────────
#[cfg(feature = "wallet")]
pub type WalletApp = wallet_app::Authenticator<TrussedClient>;
#[cfg(feature = "wallet")]
pub const WALLET_HID_MESSAGE_SIZE: usize = wallet_app::dispatch::DEFAULT_MESSAGE_SIZE;
#[cfg(feature = "wallet")]
pub type WalletHidChannel = wallet_app::dispatch::Channel<WALLET_HID_MESSAGE_SIZE>;
#[cfg(feature = "wallet")]
pub type WalletDispatch = wallet_app::dispatch::Dispatch<'static, WALLET_HID_MESSAGE_SIZE>;
#[cfg(feature = "wallet")]
pub type WalletResponder = wallet_app::dispatch::Responder<'static, WALLET_HID_MESSAGE_SIZE>;

/// NDEF-suppression clock. The NDEF app refuses `SELECT` (so phones don't pop
/// the tag during/after a FIDO ceremony) while we're within `NDEF_SUPPRESS_MS`
/// of the last FIDO command. `NDEF_CLOCK_MS` is a free-running ms counter ticked
/// by the `ndef_clock` task; `NDEF_LAST_FIDO_MS` is stamped on every FIDO
/// select/call. The clock starts at `NDEF_SUPPRESS_MS` and `NDEF_LAST_FIDO_MS`
/// at 0, so at boot the gap is already `>= NDEF_SUPPRESS_MS` (NOT suppressed) —
/// the tag stays readable until an actual FIDO command arms the window.
pub static NDEF_CLOCK_MS: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(NDEF_SUPPRESS_MS);
pub static NDEF_LAST_FIDO_MS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
const NDEF_SUPPRESS_MS: u32 = 3000;

/// nrf52840dk NDEF timebase for `solo_apps::ndef`: a free-running ms counter
/// ticked by the `ndef_clock` task. `set_fido_over_nfc` stays the default no-op
/// (the contactless-presence hook is not yet wired on the DK).
pub struct NrfNfcClock;
impl solo_apps::ndef::NfcClock for NrfNfcClock {
    const SUPPRESS_WINDOW: u32 = NDEF_SUPPRESS_MS;
    fn now() -> u32 {
        NDEF_CLOCK_MS.load(core::sync::atomic::Ordering::Relaxed)
    }
    fn last_fido() -> &'static core::sync::atomic::AtomicU32 {
        &NDEF_LAST_FIDO_MS
    }
}

use apdu_dispatch::App as ApduApp;
use ctaphid_dispatch::app::App as CtaphidApp;

static ADMIN_INTERRUPT: InterruptFlag = InterruptFlag::new();
static FIDO_INTERRUPT: InterruptFlag = InterruptFlag::new();
static SECRETS_INTERRUPT: InterruptFlag = InterruptFlag::new();
static PIV_INTERRUPT: InterruptFlag = InterruptFlag::new();
static OPCARD_INTERRUPT: InterruptFlag = InterruptFlag::new();
static NDEF_INTERRUPT: InterruptFlag = InterruptFlag::new();
#[cfg(feature = "wallet")]
static WALLET_INTERRUPT: InterruptFlag = InterruptFlag::new();
#[cfg(feature = "wallet")]
pub static WALLET_HID_CHANNEL: WalletHidChannel = WalletHidChannel::new();

/// Fire the user-cancel interrupt on every Trussed app. Trussed checks
/// these between syscalls and aborts any in-flight `confirm_user_present`
/// with `consent::Error::Interrupted`. Whichever app is currently being
/// invoked has its flag in `Working` (set by ctaphid-dispatch's call_app
/// at the host-transport layer), so the CAS in `.interrupt()` succeeds.
pub fn interrupt_all_apps() {
    ADMIN_INTERRUPT.interrupt();
    FIDO_INTERRUPT.interrupt();
    SECRETS_INTERRUPT.interrupt();
    PIV_INTERRUPT.interrupt();
    OPCARD_INTERRUPT.interrupt();
    NDEF_INTERRUPT.interrupt();
    #[cfg(feature = "wallet")]
    WALLET_INTERRUPT.interrupt();
}

/// Wallet app + its HID dispatch — intentionally NOT inside `Apps`. Its
/// sign-message blocks the calling task on `confirm_user_present`; keeping it
/// on its own resource lets the other apps run during that wait.
#[cfg(feature = "wallet")]
pub struct Wallet {
    pub app: WalletApp,
    pub dispatch: WalletDispatch,
}

/// Return-position slot for the optional Wallet: `Option<Wallet>` with the
/// feature on, unit `()` off, so the init return type is valid either way.
#[cfg(feature = "wallet")]
pub type WalletSlot = Option<Wallet>;
#[cfg(not(feature = "wallet"))]
pub type WalletSlot = ();

#[cfg(feature = "wallet")]
impl Wallet {
    pub fn new(trussed: &mut Trussed, responder: WalletResponder) -> Self {
        let client = make_client(
            client_tag::WALLET,
            littlefs2::path!("solana"),
            trussed,
            Some(&WALLET_INTERRUPT),
            &solo_apps::client::STAGING_BACKENDS,
        );
        let app = WalletApp::with_interrupt(client, Some(&WALLET_INTERRUPT));
        let dispatch = WalletDispatch::new(responder);
        Self { app, dispatch }
    }

    /// Single-shot poll of the WalletHid transport. Returns true if a response
    /// was produced (caller should pend USB).
    #[inline(never)]
    pub fn poll(&mut self) -> bool {
        self.dispatch.poll(&mut self.app)
    }
}

pub struct Apps {
    pub admin: AdminApp,
    pub fido: FidoApp,
    pub ndef: NdefApp,
    pub secrets: SecretsApp,
    pub piv: PivApp,
    pub opcard: OpcardApp,
}

impl Apps {
    pub fn new(trussed: &mut Trussed, store: RunnerStore, uuid: [u8; 16], version: u32) -> Self {
        let admin_client = make_client(
            client_tag::ADMIN,
            littlefs2::path!("admin"),
            trussed,
            Some(&ADMIN_INTERRUPT),
            &solo_apps::client::STAGING_BACKENDS,
        );
        // Load the persisted DeviceConfig (usb vid/pid/strings) so a SET_CONFIG
        // survives reboot; fall back to defaults on any error.
        let mut admin_fs = trussed::store::filestore::ClientFilestore::new(
            littlefs2::path!("admin").into(),
            store,
        );
        let admin = AdminApp::load_config(
            admin_client,
            &mut admin_fs,
            uuid,
            version,
            env!("CARGO_PKG_VERSION"),
            AdminStatus::default(),
            &[],
        )
        .unwrap_or_else(|(client, _err)| {
            AdminApp::with_default_config(
                client,
                uuid,
                version,
                env!("CARGO_PKG_VERSION"),
                AdminStatus::default(),
                &[],
            )
        });

        let fido_client = make_client(
            client_tag::FIDO,
            littlefs2::path!("fido"),
            trussed,
            Some(&FIDO_INTERRUPT),
            &solo_apps::client::STAGING_BACKENDS,
        );
        let fido = fido_authenticator::Authenticator::new(
            fido_client,
            fido_authenticator::Conforming {},
            // Shared with the lpc55 runner; `max_resident` is 50 on the DK.
            solo_apps::config::fido_config(pkg_version_u32(), 50),
        );

        let ndef_client = make_client(
            client_tag::NDEF,
            littlefs2::path!("ndef"),
            trussed,
            Some(&NDEF_INTERRUPT),
            &solo_apps::client::STAGING_BACKENDS,
        );
        let ndef = NdefApp::new(ndef_client);

        let secrets_client = make_client(
            client_tag::SECRETS,
            littlefs2::path!("secrets"),
            trussed,
            Some(&SECRETS_INTERRUPT),
            &solo_apps::client::AUTH_BACKENDS,
        );
        let secrets = SecretsApp::new(
            secrets_client,
            secrets_app::Options::new(
                trussed::types::Location::Internal,
                0, // custom_status_reverse_hotp_success
                1, // custom_status_reverse_hotp_error
                [uuid[0], uuid[1], uuid[2], uuid[3]],
                50, // max_resident_credentials_allowed
            ),
        );

        let piv_client = make_client(
            client_tag::PIV,
            littlefs2::path!("piv"),
            trussed,
            Some(&PIV_INTERRUPT),
            &solo_apps::client::PIV_BACKENDS,
        );
        let piv = PivApp::new(
            piv_client,
            piv_authenticator::Options::default().storage(trussed::types::Location::Internal),
        );

        let opcard_client = make_client(
            client_tag::OPCARD,
            littlefs2::path!("opcard"),
            trussed,
            Some(&OPCARD_INTERRUPT),
            &solo_apps::client::PIV_BACKENDS,
        );
        let opcard = {
            let mut opts = opcard::Options::default();
            opts.storage = trussed::types::Location::Internal;
            OpcardApp::new(opcard_client, opts)
        };

        Self {
            admin,
            fido,
            ndef,
            secrets,
            piv,
            opcard,
        }
    }

    /// Apps that handle CTAPHID frames (fido + admin + secrets via 0x70..).
    #[inline(never)]
    pub fn ctaphid_dispatch<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut [&mut dyn CtaphidApp<'static>]) -> T,
    {
        f(&mut [&mut self.admin, &mut self.fido, &mut self.secrets])
    }

    /// Apps that handle APDUs over CCID/NFC. More specific AIDs first.
    ///
    /// `secrets` (Yubico OATH AID `A0 00 00 05 27 21 01`) is intentionally
    /// omitted from the NFC slate: some phone NFC stacks match it before
    /// reaching the FIDO AID, which prevents WebAuthn flows from entering
    /// the FIDO dialog. It's still reachable via CTAPHID over USB.
    #[inline(never)]
    pub fn apdu_dispatch<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut [&mut dyn ApduApp]) -> T,
    {
        f(&mut [
            &mut NdefFidoGate::<NrfNfcClock, _>::new(&mut self.ndef),
            &mut self.piv,
            &mut self.opcard,
            &mut FidoNdefStamp::<NrfNfcClock, _>::new(&mut self.fido),
            &mut self.admin,
        ])
    }
}
