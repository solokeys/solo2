//! Trussed extension dispatch.
//!
//! Wires the StagingBackend (Hkdf, FsInfo, Manage, Chunked, Hpke,
//! WrapKeyToFile) and AuthBackend extensions used by fido-authenticator,
//! admin-app, secrets-app, piv-authenticator, and opcard.

use trussed::serde_extensions::{ExtensionDispatch, ExtensionId, ExtensionImpl};
use trussed_auth::AuthExtension;
use trussed_auth_backend::{AuthBackend, AuthContext, FilesystemLayout};
use trussed_chunked::ChunkedExtension;
use trussed_fs_info::FsInfoExtension;
use trussed_hkdf::HkdfExtension;
use trussed_hpke::HpkeExtension;
use trussed_manage::ManageExtension;
use trussed_staging::{StagingBackend, StagingContext};
use trussed_wrap_key_to_file::WrapKeyToFileExtension;

pub struct Dispatch {
    staging_backend: StagingBackend,
    auth_backend: AuthBackend,
}

impl Default for Dispatch {
    fn default() -> Self {
        Self {
            staging_backend: StagingBackend::new(),
            auth_backend: AuthBackend::new(
                trussed_core::types::Location::Internal,
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
    type Error = trussed_core::Error;
    fn try_from(id: u8) -> Result<Self, trussed_core::Error> {
        match id {
            0 => Ok(Self::Auth),
            1 => Ok(Self::Hkdf),
            2 => Ok(Self::Manage),
            3 => Ok(Self::WrapKeyToFile),
            4 => Ok(Self::FsInfo),
            5 => Ok(Self::Hpke),
            6 => Ok(Self::Chunked),
            _ => Err(trussed_core::Error::FunctionNotSupported),
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
        ctx: &mut trussed::types::Context<Self::Context>,
        request: &trussed_core::api::Request,
        resources: &mut trussed::service::ServiceResources<P>,
    ) -> Result<trussed_core::api::Reply, trussed_core::Error> {
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
        request: &trussed_core::api::request::SerdeExtension,
        resources: &mut trussed::service::ServiceResources<P>,
    ) -> Result<trussed_core::api::reply::SerdeExtension, trussed_core::Error> {
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
            ExtensionIds::Hpke => ExtensionImpl::<HpkeExtension>::extension_request_serialized(
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
            ExtensionIds::WrapKeyToFile => {
                ExtensionImpl::<WrapKeyToFileExtension>::extension_request_serialized(
                    &mut self.staging_backend,
                    &mut ctx.core,
                    &mut ctx.backends.staging,
                    request,
                    resources,
                )
            }
            ExtensionIds::Chunked => {
                ExtensionImpl::<ChunkedExtension>::extension_request_serialized(
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
