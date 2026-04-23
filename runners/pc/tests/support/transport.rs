//! Transport abstraction for FIDO2 tests.
//!
//! - Default: in-process simulator (calls `fido_authenticator::Authenticator::call_ctap2` directly)
//! - `FIDO2_TRANSPORT=socket`: Unix socket to a running PC-runner simulator
//! - `FIDO2_TRANSPORT=device`: USB HID to a real FIDO2 device
//!
//! NOTE: `ctap_types 0.5` `Request<'a>` only implements `DeserializeIndexed` (for
//! authenticator-side parsing); it does not implement `Serialize`. Hence the
//! Device/Socket host-side paths need a manual request→CBOR encoder that this
//! file no longer provides — those backends currently panic at first use.

use ctap_types::ctap2::{self, Request, Response};

/// The single interface tests use to talk to any authenticator backend.
pub trait TestAuthenticator {
    fn call_ctap2(&mut self, request: &Request) -> Result<Response, ctap2::Error>;
    fn call_ctap2_raw(
        &mut self,
        command: u8,
        payload: &[u8],
    ) -> Result<(u8, Vec<u8>), ctap2::Error>;
    /// Reconnect to the device after a reboot. No-op for in-process backends.
    fn reconnect(&mut self) {}
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Backend {
    Sim,
    Socket,
    Device,
}

pub fn backend() -> Backend {
    match std::env::var("FIDO2_TRANSPORT").as_deref() {
        Ok("device") => Backend::Device,
        Ok("socket") => Backend::Socket,
        _ => Backend::Sim,
    }
}

/// Returns true if tests should target a real USB device.
pub fn is_device_mode() -> bool {
    backend() == Backend::Device
}

// ---------------------------------------------------------------------------
// Sim backend: any `fido_authenticator::Authenticator` is a `TestAuthenticator`
// ---------------------------------------------------------------------------

impl<UP, T> TestAuthenticator for fido_authenticator::Authenticator<UP, T>
where
    UP: fido_authenticator::UserPresence,
    T: fido_authenticator::TrussedRequirements,
{
    fn call_ctap2(&mut self, request: &Request) -> Result<Response, ctap2::Error> {
        ctap2::Authenticator::call_ctap2(self, request)
    }

    fn call_ctap2_raw(
        &mut self,
        command: u8,
        payload: &[u8],
    ) -> Result<(u8, Vec<u8>), ctap2::Error> {
        use ctaphid_dispatch::app::{App, Command};

        // Build the request as raw bytes.
        let mut request = heapless::Vec::<u8, 3072>::new();
        request
            .push(command)
            .map_err(|_| ctap2::Error::RequestTooLarge)?;
        request
            .extend_from_slice(payload)
            .map_err(|_| ctap2::Error::RequestTooLarge)?;

        // `ctaphid_app::App::call` writes into `&mut BytesView`. We obtain one
        // by coercing an owned `Bytes<N>` via `as_mut_view()`.
        let mut backing = ctap_types::heapless_bytes::Bytes::<3072>::new();
        App::call(
            self,
            Command::Cbor,
            request.as_slice(),
            backing.as_mut_view(),
        )
        .map_err(|_| ctap2::Error::Other)?;

        if backing.is_empty() {
            return Err(ctap2::Error::Other);
        }
        Ok((backing[0], backing[1..].to_vec()))
    }
}

// ---------------------------------------------------------------------------
// Device/socket backend: CTAPHID (stubbed — see note at top of file)
// ---------------------------------------------------------------------------

pub struct DeviceTransport {
    client: super::ctaphid::CtapHidClient,
}

impl DeviceTransport {
    pub fn open_hid() -> Self {
        Self {
            client: super::ctaphid::CtapHidClient::open_hid(),
        }
    }

    pub fn open_socket() -> Self {
        Self {
            client: super::ctaphid::CtapHidClient::connect_socket(),
        }
    }
}

impl TestAuthenticator for DeviceTransport {
    fn call_ctap2(&mut self, _request: &Request) -> Result<Response, ctap2::Error> {
        unimplemented!(
            "DeviceTransport::call_ctap2 needs a host-side Request→CBOR serializer \
             (ctap-types 0.5 Request<'_> is deserialize-only)."
        );
    }

    fn call_ctap2_raw(
        &mut self,
        command: u8,
        payload: &[u8],
    ) -> Result<(u8, Vec<u8>), ctap2::Error> {
        let mut data = Vec::with_capacity(1 + payload.len());
        data.push(command);
        data.extend_from_slice(payload);
        self.client
            .ctap2(&data, std::time::Duration::from_secs(30))
            .map_err(|_| ctap2::Error::Other)
    }

    fn reconnect(&mut self) {
        self.client = super::ctaphid::CtapHidClient::open_hid();
    }
}

pub fn error_from_byte(b: u8) -> ctap2::Error {
    match b {
        0x01 => ctap2::Error::InvalidCommand,
        0x02 => ctap2::Error::InvalidParameter,
        0x03 => ctap2::Error::InvalidLength,
        0x14 => ctap2::Error::MissingParameter,
        0x19 => ctap2::Error::CredentialExcluded,
        0x22 => ctap2::Error::InvalidCredential,
        0x26 => ctap2::Error::UnsupportedAlgorithm,
        0x27 => ctap2::Error::OperationDenied,
        0x2E => ctap2::Error::NoCredentials,
        0x2F => ctap2::Error::UserActionTimeout,
        0x30 => ctap2::Error::NotAllowed,
        0x31 => ctap2::Error::PinInvalid,
        0x32 => ctap2::Error::PinBlocked,
        0x33 => ctap2::Error::PinAuthInvalid,
        0x34 => ctap2::Error::PinAuthBlocked,
        0x35 => ctap2::Error::PinNotSet,
        0x36 => ctap2::Error::PinRequired,
        0x37 => ctap2::Error::PinPolicyViolation,
        0x38 => ctap2::Error::PinTokenExpired,
        0x3B => ctap2::Error::UpRequired,
        _ => ctap2::Error::Other,
    }
}
