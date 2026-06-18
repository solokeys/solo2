//! Shared trussed client plumbing — backend slates, the multiplexed service
//! wrapper, per-app client tags, and client registration.
//!
//! Generic over the runner's `Board` (a trussed `Platform`) and `Syscall` so the
//! board-specific pieces stay in each runner.

use crate::dispatch::{BackendIds, Dispatch, RunnerContext};
use trussed::backend::BackendId;
use trussed::client::{ClientTag, CurrentTagCell, MultiplexedClient, SharedRequesterCell, Syscall};
use trussed::interrupt::InterruptFlag;
use trussed::pipe::{MultiplexedEndpoint, TrussedChannel};
use trussed::platform::Platform;
use trussed::types::{Context, CoreContext};

/// Apps that need only StagingBackend (FsInfo, Hkdf, Manage) + Core.
/// `BackendId::Core` must be present or standard crypto/filesystem calls return
/// RequestNotAvailable and `syscall!()` panics.
pub static STAGING_BACKENDS: [BackendId<BackendIds>; 2] = [
    BackendId::Custom(BackendIds::StagingBackend),
    BackendId::Core,
];

/// secrets-app needs the Auth extension (PIN management). Auth must come first.
pub static AUTH_BACKENDS: [BackendId<BackendIds>; 2] =
    [BackendId::Custom(BackendIds::Auth), BackendId::Core];

/// piv-authenticator and opcard need Auth + Staging + Core.
pub static PIV_BACKENDS: [BackendId<BackendIds>; 3] = [
    BackendId::Custom(BackendIds::Auth),
    BackendId::Custom(BackendIds::StagingBackend),
    BackendId::Core,
];

/// Multiplexed service endpoint: one shared responder, per-client contexts
/// keyed by `ClientTag`.
pub type TrussedEndpoint = MultiplexedEndpoint<'static, BackendIds, RunnerContext>;

/// Single channel shared across every app. The requester half is stashed in
/// `SHARED_REQUESTER` so all `MultiplexedClient`s submit through it; the
/// responder half is owned by the `MultiplexedEndpoint` inside `Trussed` and
/// pumped by `Service::process_multiplexed`.
static SHARED_TRUSSED_CHANNEL: TrussedChannel = TrussedChannel::new();
static SHARED_REQUESTER: SharedRequesterCell = SharedRequesterCell::new();
/// Set by whichever client most recently submitted a request; read by the
/// service to find the matching context.
static CURRENT_TAG: CurrentTagCell = CurrentTagCell::new();

/// Per-app `ClientTag` (1..=N; 0 reserved as "no client"). Each value must be
/// distinct so `process_multiplexed` routes each request to the right context.
/// The nrf52840dk runner uses the first six; provisioner/oath-export are lpc55-only.
#[allow(dead_code)]
pub mod client_tag {
    use super::ClientTag;
    pub const ADMIN: ClientTag = 1;
    pub const FIDO: ClientTag = 2;
    pub const NDEF: ClientTag = 3;
    pub const SECRETS: ClientTag = 4;
    pub const PIV: ClientTag = 5;
    pub const OPCARD: ClientTag = 6;
    pub const PROVISIONER: ClientTag = 7;
    pub const OATH_EXPORT: ClientTag = 8;
}

/// Wrapper around the trussed `Service` that owns the multiplexed endpoint.
/// `process()` and `update_ui()` are called from the runner's RTIC OS-event
/// handler and the periodic UI task respectively.
pub struct Trussed<B: Platform> {
    service: trussed::Service<B, Dispatch>,
    endpoint: TrussedEndpoint,
}

impl<B: Platform> Trussed<B> {
    pub fn new(service: trussed::Service<B, Dispatch>) -> Self {
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
        context: Context<RunnerContext>,
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

/// Register a multiplexed client with the shared trussed service and return its
/// `MultiplexedClient`. The runner contributes the per-app `tag`, `client_id`
/// directory, optional `interrupt`, and the backends list routing extension calls.
pub fn make_client<B: Platform, S: Syscall + Default>(
    tag: ClientTag,
    client_id: &'static littlefs2::path::Path,
    trussed: &mut Trussed<B>,
    interrupt: Option<&'static InterruptFlag>,
    backends: &'static [BackendId<BackendIds>],
) -> MultiplexedClient<S, Dispatch> {
    let context = CoreContext::with_interrupt(littlefs2::path::PathBuf::from(client_id), interrupt);
    trussed.register_client(tag, context.into(), backends);
    MultiplexedClient::new(
        &SHARED_REQUESTER,
        &CURRENT_TAG,
        tag,
        S::default(),
        interrupt,
    )
}
