//! In-process simulator harness for FIDO2 tests.
//!
//! Spawns a `trussed::Service` on a scoped thread and runs a test closure that
//! gets a `ClientImplementation`. Provides the extension backends
//! (`trussed-staging` + `trussed-auth-backend`) that `fido-authenticator 0.3`
//! requires via its `TrussedRequirements` trait bound.
//!
//! User-presence control uses `solo_pc::buttons::TestThreeButtons` (`test-buttons` feature),
//! mirroring the embedded `Press` + `Edge` button path in `board::trussed::UserInterface`.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use trussed::backend::{Backend, BackendId};
use trussed::pipe::{ServiceEndpoint, TrussedChannel};
use trussed::serde_extensions::{ExtensionDispatch, ExtensionId, ExtensionImpl};
use trussed::service::Service;
use trussed::types::{Context, CoreContext};
use trussed::ClientImplementation;

use trussed_auth::AuthExtension;
use trussed_auth_backend::{AuthBackend, AuthContext, FilesystemLayout};
use trussed_chunked::ChunkedExtension;
use trussed_fs_info::FsInfoExtension;
use trussed_hkdf::HkdfExtension;
use trussed_hpke::HpkeExtension;
use trussed_manage::ManageExtension;
use trussed_staging::{StagingBackend, StagingContext};
use trussed_wrap_key_to_file::WrapKeyToFileExtension;

// ---------------------------------------------------------------------------
// Dispatch (mirror of lpc55's)
// ---------------------------------------------------------------------------

pub struct Dispatch {
    staging_backend: StagingBackend,
    auth_backend: AuthBackend,
}

impl Default for Dispatch {
    fn default() -> Self {
        Self {
            staging_backend: StagingBackend::new(),
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
        ctx: &mut Context<Self::Context>,
        request: &trussed::api::Request,
        resources: &mut trussed::service::ServiceResources<P>,
    ) -> Result<trussed::Reply, trussed::Error> {
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
        ctx: &mut Context<Self::Context>,
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

// ---------------------------------------------------------------------------
// mpsc Syscall
// ---------------------------------------------------------------------------

pub struct Syscall {
    tx: Sender<()>,
}

impl trussed::platform::Syscall for Syscall {
    fn syscall(&mut self) {
        self.tx.send(()).unwrap();
    }
}

pub type TestClient<'a> = ClientImplementation<'a, Syscall, Dispatch>;

/// Run `f` with a freshly-constructed in-process Trussed client. The Service
/// runs on a scoped thread; when `f` returns, the service is stopped cleanly.
pub fn with_client<R>(f: impl FnOnce(TestClient<'_>) -> R + Send) -> R
where
    R: Send,
{
    let channel = TrussedChannel::new();
    let (requester, responder) = channel.split().unwrap();

    let (syscall_tx, syscall_rx): (Sender<()>, Receiver<()>) = mpsc::channel();
    let syscall = Syscall { tx: syscall_tx };

    let store = solo_pc::mount_filesystems();
    let rng = <chacha20::ChaCha8Rng as rand_core::SeedableRng>::from_seed([0u8; 32]);
    let platform = solo_pc::Board::new(rng, store, solo_pc::UserInterface::default());

    static BACKENDS: &[BackendId<BackendIds>] = &[
        BackendId::Custom(BackendIds::Auth),
        BackendId::Custom(BackendIds::StagingBackend),
        BackendId::Core,
    ];

    let mut endpoints: [ServiceEndpoint<'_, BackendIds, RunnerContext>; 1] =
        [ServiceEndpoint::new(
            responder,
            CoreContext::new(littlefs2::path!("fido").into()),
            BACKENDS,
        )];

    let dispatch = Dispatch::default();
    let mut service = Service::with_dispatch(platform, dispatch);

    thread::scope(|s| {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let handle = s.spawn(move || {
            loop {
                // Give the syscall channel priority; exit when main thread signals done.
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                match syscall_rx.recv_timeout(std::time::Duration::from_millis(10)) {
                    Ok(()) => service.process(&mut endpoints),
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        let client = ClientImplementation::new(requester, syscall, None);
        let result = f(client);
        let _ = stop_tx.send(());
        let _ = handle.join();
        result
    })
}
