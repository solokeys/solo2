include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

use crate::hal;
use hal::drivers::timer;
use hal::peripherals::ctimer;
use littlefs2::{const_ram_storage, consts};
use trussed::backend::BackendId;
use trussed::client::{ClientTag, CurrentTagCell, MultiplexedClient, SharedRequesterCell};
use trussed::interrupt::InterruptFlag;
use trussed::pipe::{MultiplexedEndpoint, TrussedChannel};
use trussed::platform;
use trussed::serde_extensions::{ExtensionDispatch, ExtensionId, ExtensionImpl};
use trussed::store::DynFilesystem;
use trussed::types::CoreContext;
use trussed_auth::AuthExtension;
use trussed_auth_backend::{AuthBackend, AuthContext, FilesystemLayout};
use trussed_chunked::ChunkedExtension;
use trussed_fs_info::FsInfoExtension;
use trussed_hkdf::HkdfExtension;
use trussed_hpke::HpkeExtension;
use trussed_manage::ManageExtension;
use trussed_staging::{StagingBackend, StagingContext};
use trussed_wrap_key_to_file::WrapKeyToFileExtension;

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
// fallback is only used when no chip is present (e.g. a bare EVK). On such
// boards enlarge it so the migration import test can actually store creds.
#[cfg(not(feature = "no-encrypted-storage"))]
const_ram_storage!(ExternalFallbackStorage, 4096);
#[cfg(feature = "no-encrypted-storage")]
const_ram_storage!(ExternalFallbackStorage, 32768);

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

/// Extension dispatch type providing FsInfo, Hkdf, Manage (via trussed-staging) and
/// Auth (via trussed-auth-backend) extensions.
/// Required because fido-authenticator 0.2 unconditionally needs FsInfoClient + HkdfClient,
/// admin-app requires ManageClient, and secrets-app requires AuthClient.
pub struct Dispatch {
    staging_backend: StagingBackend,
    auth_backend: AuthBackend,
}

impl Default for Dispatch {
    fn default() -> Self {
        Self {
            staging_backend: StagingBackend::new(),
            // V0 layout: new device, no existing auth data to migrate
            auth_backend: AuthBackend::new(
                trussed::types::Location::Internal,
                FilesystemLayout::V0,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendIds {
    StagingBackend,
    Auth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionIds {
    Auth = 0,
    Hkdf = 1,
    Manage = 2,
    WrapKeyToFile = 3,
    FsInfo = 4,
    Hpke = 5,
    Chunked = 6,
}

impl From<ExtensionIds> for u8 {
    fn from(id: ExtensionIds) -> u8 {
        id as u8
    }
}

impl TryFrom<u8> for ExtensionIds {
    type Error = trussed::Error;
    fn try_from(id: u8) -> Result<Self, trussed::Error> {
        match id {
            0 => Ok(Self::Auth),
            1 => Ok(Self::Hkdf),
            2 => Ok(Self::Manage),
            3 => Ok(Self::WrapKeyToFile),
            4 => Ok(Self::FsInfo),
            5 => Ok(Self::Hpke),
            6 => Ok(Self::Chunked),
            _ => Err(trussed::Error::FunctionNotSupported),
        }
    }
}

impl ExtensionId<AuthExtension> for Dispatch {
    type Id = ExtensionIds;
    const ID: ExtensionIds = ExtensionIds::Auth;
}

impl ExtensionId<ChunkedExtension> for Dispatch {
    type Id = ExtensionIds;
    const ID: ExtensionIds = ExtensionIds::Chunked;
}

impl ExtensionId<FsInfoExtension> for Dispatch {
    type Id = ExtensionIds;
    const ID: ExtensionIds = ExtensionIds::FsInfo;
}

impl ExtensionId<HkdfExtension> for Dispatch {
    type Id = ExtensionIds;
    const ID: ExtensionIds = ExtensionIds::Hkdf;
}

impl ExtensionId<HpkeExtension> for Dispatch {
    type Id = ExtensionIds;
    const ID: ExtensionIds = ExtensionIds::Hpke;
}

impl ExtensionId<ManageExtension> for Dispatch {
    type Id = ExtensionIds;
    const ID: ExtensionIds = ExtensionIds::Manage;
}

impl ExtensionId<WrapKeyToFileExtension> for Dispatch {
    type Id = ExtensionIds;
    const ID: ExtensionIds = ExtensionIds::WrapKeyToFile;
}

/// Combined context for all backends in the dispatch.
#[derive(Default)]
pub struct RunnerContext {
    pub auth: AuthContext,
    pub staging: StagingContext,
}

impl ExtensionDispatch for Dispatch {
    type BackendId = BackendIds;
    type Context = RunnerContext;
    type ExtensionId = ExtensionIds;

    fn core_request<P: trussed::platform::Platform>(
        &mut self,
        backend: &Self::BackendId,
        ctx: &mut trussed::types::Context<Self::Context>,
        request: &trussed::api::Request,
        resources: &mut trussed::service::ServiceResources<P>,
    ) -> Result<trussed::Reply, trussed::Error> {
        use trussed::backend::Backend;
        match backend {
            BackendIds::StagingBackend => self.staging_backend.request(
                &mut ctx.core,
                &mut ctx.backends.staging,
                request,
                resources,
            ),
            BackendIds::Auth => {
                self.auth_backend
                    .request(&mut ctx.core, &mut ctx.backends.auth, request, resources)
            }
        }
    }

    fn extension_request<P: trussed::platform::Platform>(
        &mut self,
        _backend: &Self::BackendId,
        extension: &Self::ExtensionId,
        ctx: &mut trussed::types::Context<Self::Context>,
        request: &trussed::api::request::SerdeExtension,
        resources: &mut trussed::service::ServiceResources<P>,
    ) -> Result<trussed::api::reply::SerdeExtension, trussed::Error> {
        match extension {
            ExtensionIds::Auth => self.auth_backend.extension_request_serialized(
                &mut ctx.core,
                &mut ctx.backends.auth,
                request,
                resources,
            ),
            ExtensionIds::FsInfo => ExtensionImpl::<FsInfoExtension>::extension_request_serialized(
                &mut self.staging_backend,
                &mut ctx.core,
                &mut ctx.backends.staging,
                request,
                resources,
            ),
            ExtensionIds::Hkdf => ExtensionImpl::<HkdfExtension>::extension_request_serialized(
                &mut self.staging_backend,
                &mut ctx.core,
                &mut ctx.backends.staging,
                request,
                resources,
            ),
            ExtensionIds::Manage => ExtensionImpl::<ManageExtension>::extension_request_serialized(
                &mut self.staging_backend,
                &mut ctx.core,
                &mut ctx.backends.staging,
                request,
                resources,
            ),
            ExtensionIds::Chunked => {
                ExtensionImpl::<ChunkedExtension>::extension_request_serialized(
                    &mut self.staging_backend,
                    &mut ctx.core,
                    &mut ctx.backends.staging,
                    request,
                    resources,
                )
            }
            ExtensionIds::Hpke => ExtensionImpl::<HpkeExtension>::extension_request_serialized(
                &mut self.staging_backend,
                &mut ctx.core,
                &mut ctx.backends.staging,
                request,
                resources,
            ),
            ExtensionIds::WrapKeyToFile => {
                ExtensionImpl::<WrapKeyToFileExtension>::extension_request_serialized(
                    &mut self.staging_backend,
                    &mut ctx.core,
                    &mut ctx.backends.staging,
                    request,
                    resources,
                )
            }
        }
    }
}

#[derive(Default)]
pub struct Syscall {}

impl trussed::client::Syscall for Syscall {
    #[inline]
    fn syscall(&mut self) {
        rtic::pend(board::hal::raw::Interrupt::OS_EVENT);
    }
}

/// Multiplexed service endpoint: one shared responder, per-client contexts
/// keyed by `ClientTag`.
pub type TrussedEndpoint = MultiplexedEndpoint<'static, BackendIds, RunnerContext>;
/// Client type for apps — all apps share the SHARED_TRUSSED_CHANNEL and are
/// distinguished by a per-app `ClientTag` so the service can route requests
/// to the matching context.
pub type TrussedClient = MultiplexedClient<Syscall, Dispatch>;

/// Backends for most apps: StagingBackend (FsInfo, Hkdf, Manage) + Core.
/// BackendId::Core must be present or all standard crypto/filesystem calls return
/// RequestNotAvailable, causing syscall!() to panic and the device to freeze.
static STAGING_BACKENDS: [BackendId<BackendIds>; 2] = [
    BackendId::Custom(BackendIds::StagingBackend),
    BackendId::Core,
];

/// Backends for apps requiring Auth extension (secrets-app).
/// Auth must come first so AuthClient calls reach AuthBackend.
#[cfg(feature = "oath")]
static AUTH_BACKENDS: [BackendId<BackendIds>; 2] =
    [BackendId::Custom(BackendIds::Auth), BackendId::Core];

/// Backends for piv-authenticator: needs Auth (PIN management), Staging (Chunked/Hpke/WrapKeyToFile),
/// and Core (standard crypto/filesystem).
#[cfg(feature = "piv-authenticator")]
static PIV_BACKENDS: [BackendId<BackendIds>; 3] = [
    BackendId::Custom(BackendIds::Auth),
    BackendId::Custom(BackendIds::StagingBackend),
    BackendId::Core,
];

/// Backends for opcard: same requirements as PIV (Auth for PIN, Staging for Chunked/WrapKeyToFile,
/// Core for standard crypto/filesystem).
#[cfg(feature = "opcard")]
static OPCARD_BACKENDS: [BackendId<BackendIds>; 3] = [
    BackendId::Custom(BackendIds::Auth),
    BackendId::Custom(BackendIds::StagingBackend),
    BackendId::Core,
];

/// Wrapper around the trussed Service that owns the multiplexed endpoint.
/// `process()` and `update_ui()` are called from the RTIC OS_EVENT handler and
/// the periodic UI task respectively.
pub struct Trussed {
    service: trussed::Service<Board, Dispatch>,
    endpoint: TrussedEndpoint,
}

impl Trussed {
    pub fn new(service: trussed::Service<Board, Dispatch>) -> Self {
        let (req, resp) = SHARED_TRUSSED_CHANNEL
            .split()
            .expect("shared trussed channel already split");
        SHARED_REQUESTER.init(req);
        Self {
            service,
            endpoint: MultiplexedEndpoint::new(resp),
        }
    }

    pub fn register_client(
        &mut self,
        tag: ClientTag,
        context: trussed::types::Context<RunnerContext>,
        backends: &'static [BackendId<BackendIds>],
    ) {
        self.endpoint
            .register((tag, context, backends))
            .map_err(|_| ())
            .expect("MultiplexedEndpoint full");
    }

    pub fn process(&mut self) {
        self.service
            .process_multiplexed(&mut self.endpoint, &CURRENT_TAG);
    }

    pub fn update_ui(&mut self) {
        self.service.update_ui();
    }
}

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
#[cfg(feature = "ndef-app")]
pub type NdefApp = ndef_app::App<'static>;
#[cfg(feature = "provisioner-app")]
pub type ProvisionerApp = provisioner_app::Provisioner<Store, FlashStorage, TrussedClient>;

use apdu_dispatch::App as ApduApp;
use ctaphid_dispatch::app::App as CtaphidApp;

pub type DynamicClockController = board::clock_controller::DynamicClockController;
pub type NfcWaitExtender = timer::Timer<ctimer::Ctimer0<hal::typestates::init_state::Enabled>>;
pub type PerformanceTimer = timer::Timer<ctimer::Ctimer4<hal::typestates::init_state::Enabled>>;

/// Single channel shared across every app. The requester half is stashed in
/// `SHARED_REQUESTER` so all `MultiplexedClient`s can submit requests through
/// it; the responder half is owned by the `MultiplexedEndpoint` inside
/// `Trussed` and pumped by `Service::process_multiplexed`.
static SHARED_TRUSSED_CHANNEL: TrussedChannel = TrussedChannel::new();
static SHARED_REQUESTER: SharedRequesterCell = SharedRequesterCell::new();
/// Set by whichever client most recently submitted a request; read by the
/// service to find the matching context.
static CURRENT_TAG: CurrentTagCell = CurrentTagCell::new();

/// Per-app ClientTag (1..=N; 0 reserved as "no client"). Each value must be
/// distinct so `process_multiplexed` can route requests to the right context.
#[allow(dead_code)]
mod client_tag {
    use super::ClientTag;
    pub const ADMIN: ClientTag = 1;
    pub const FIDO: ClientTag = 2;
    pub const SECRETS: ClientTag = 4;
    pub const PIV: ClientTag = 5;
    pub const OPCARD: ClientTag = 6;
    pub const PROVISIONER: ClientTag = 7;
    pub const OATH_EXPORT: ClientTag = 8;
}

#[cfg(feature = "admin-app")]
static ADMIN_INTERRUPT: InterruptFlag = InterruptFlag::new();
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

/// Register a multiplexed client with the shared trussed service and return
/// the corresponding `MultiplexedClient`. The runner contributes the
/// per-app `tag`, `client_id` directory, optional `interrupt`, and the
/// backends list used to route extension calls.
fn make_client(
    tag: ClientTag,
    client_id: &'static littlefs2::path::Path,
    trussed: &mut Trussed,
    interrupt: Option<&'static InterruptFlag>,
    backends: &'static [BackendId<BackendIds>],
) -> TrussedClient {
    let context = CoreContext::with_interrupt(littlefs2::path::PathBuf::from(client_id), interrupt);
    trussed.register_client(tag, context.into(), backends);
    MultiplexedClient::new(
        &SHARED_REQUESTER,
        &CURRENT_TAG,
        tag,
        Syscall::default(),
        interrupt,
    )
}

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
                &STAGING_BACKENDS,
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
                &STAGING_BACKENDS,
            );
            fido_authenticator::Authenticator::new(
                client,
                fido_authenticator::Conforming {},
                FidoConfig {
                    max_msg_size: ctaphid_dispatch::DEFAULT_MESSAGE_SIZE,
                    skip_up_timeout: None,
                    max_resident_credential_count: Some(100),
                    // CTAP 2.1 §6.10: minimum array size is 1024.
                    large_blobs: Some(fido_authenticator::LargeBlobsConfig {
                        location: trussed::types::Location::External,
                        max_size: 1024,
                    }),
                    nfc_transport: true,
                    ccid_transport: false,
                    firmware_version: Some((build_constants::CARGO_PKG_VERSION as usize).into()),
                    // V2 credential-id format: AES-256-GCM. Applied on a clean
                    // state / after factory reset; existing V1 credentials persist.
                    credential_id_version: Some(
                        fido_authenticator::credential::CredentialIdVersion::V2,
                    ),
                    long_touch_for_reset: true,
                    fido2_up_timeout: None,
                },
            )
        };

        #[cfg(feature = "piv-authenticator")]
        let piv = {
            let client = make_client(
                client_tag::PIV,
                littlefs2::path!("piv"),
                trussed,
                Some(&PIV_INTERRUPT),
                &PIV_BACKENDS,
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
                &OPCARD_BACKENDS,
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
                &AUTH_BACKENDS,
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
        let ndef = NdefApp::new();

        #[cfg(feature = "provisioner-app")]
        let provisioner = {
            let client = make_client(
                client_tag::PROVISIONER,
                littlefs2::path!("attn"),
                trussed,
                Some(&PROVISIONER_INTERRUPT),
                &STAGING_BACKENDS,
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
                &STAGING_BACKENDS,
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
            &mut self.ndef,
            #[cfg(feature = "piv-authenticator")]
            &mut self.piv,
            #[cfg(feature = "opcard")]
            &mut self.opcard,
            #[cfg(feature = "oath")]
            &mut self.secrets,
            #[cfg(feature = "fido-authenticator")]
            &mut self.fido,
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
