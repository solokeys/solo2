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
    /// Submit a CTAP1 (U2F) APDU via CTAPHID `MSG` (0x83). Used by `tests/u2f.rs`.
    /// Returns `(sw1<<8 | sw2, payload)` so callers can match on the U2F
    /// status word (`APDU_NO_ERROR = 0x9000`, `WRONG_DATA = 0x6A80`,
    /// `USE_NOT_SATISFIED = 0x6985`, etc.).
    fn call_ctap1_apdu(&mut self, apdu: &[u8]) -> Result<(u16, Vec<u8>), ctap2::Error>;
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
        let mut response = Response::Reset;
        ctap2::Authenticator::call_ctap2(self, request, &mut response)?;
        Ok(response)
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

    fn call_ctap1_apdu(&mut self, apdu: &[u8]) -> Result<(u16, Vec<u8>), ctap2::Error> {
        use ctaphid_dispatch::app::{App, Command};

        let mut backing = ctap_types::heapless_bytes::Bytes::<3072>::new();
        App::call(self, Command::Msg, apdu, backing.as_mut_view())
            .map_err(|_| ctap2::Error::Other)?;

        // CTAP1 responses end with a 2-byte status word (SW1 SW2). The
        // payload is everything before those two bytes.
        let n = backing.len();
        if n < 2 {
            return Err(ctap2::Error::Other);
        }
        let sw = u16::from_be_bytes([backing[n - 2], backing[n - 1]]);
        let payload = backing[..n - 2].to_vec();
        Ok((sw, payload))
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

/// `TEST_DELAY_MS=N` sleeps this many ms BEFORE every CTAP2 / raw CTAP /
/// CTAP1 transport call. Use to debug whether host-side sustained-traffic
/// drops (VM USB pass-through is sensitive) ease up with breathing room.
fn pre_call_delay() {
    if let Ok(v) = std::env::var("TEST_DELAY_MS") {
        if let Ok(ms) = v.parse::<u64>() {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }
}

impl TestAuthenticator for DeviceTransport {
    fn call_ctap2(&mut self, request: &Request) -> Result<Response, ctap2::Error> {
        pre_call_delay();
        // Stack-allocated CBOR scratch big enough for CTAP-2.3 ML-DSA-44
        // sized requests (~4 KiB ceiling for MakeCredential w/ large lists).
        let mut buf = vec![0u8; 4096];
        let total_len = serialize_request(request, &mut buf)?;
        let result = self
            .client
            .ctap2(&buf[..total_len], std::time::Duration::from_secs(45));
        let (status, cbor) = match result {
            Ok(out) => out,
            Err(e) => {
                eprintln!("[transport] CTAPHID error: {e}");
                // USB pass-through quirk: under sustained CTAPHID load,
                // the host kernel can mark the device as `disconnected`
                // mid-transaction even though the chip's USB peripheral
                // is fine. `TEST_DK_RECOVER=1` opens a fresh hidraw
                // handle and retries the request once before failing.
                if std::env::var("TEST_DK_RECOVER").is_ok()
                    && e.to_string().contains("device disconnected")
                {
                    eprintln!("[transport] reopening HID and retrying once");
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    self.reconnect();
                    let retry = self
                        .client
                        .ctap2(&buf[..total_len], std::time::Duration::from_secs(45));
                    match retry {
                        Ok(out) => out,
                        Err(e2) => {
                            eprintln!("[transport] retry also failed: {e2}");
                            return Err(ctap2::Error::Other);
                        }
                    }
                } else {
                    return Err(ctap2::Error::Other);
                }
            }
        };
        if status != 0 {
            return Err(error_from_byte(status));
        }
        deserialize_response(request, &cbor)
    }

    fn call_ctap2_raw(
        &mut self,
        command: u8,
        payload: &[u8],
    ) -> Result<(u8, Vec<u8>), ctap2::Error> {
        pre_call_delay();
        let mut data = Vec::with_capacity(1 + payload.len());
        data.push(command);
        data.extend_from_slice(payload);
        self.client
            .ctap2(&data, std::time::Duration::from_secs(30))
            .map_err(|_| ctap2::Error::Other)
    }

    fn call_ctap1_apdu(&mut self, apdu: &[u8]) -> Result<(u16, Vec<u8>), ctap2::Error> {
        pre_call_delay();
        self.client
            .ctap1(apdu, std::time::Duration::from_secs(30))
            .map_err(|e| {
                eprintln!("[transport] CTAP1 error: {e}");
                ctap2::Error::Other
            })
    }

    fn reconnect(&mut self) {
        // After `probe-rs reset` the chip re-enumerates over USB. On
        // VMs/hypervisors that need a re-attach hook after USB
        // re-enumeration, the device can be briefly absent. Retry a few
        // times so the host has a chance to rebind it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        loop {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                super::ctaphid::CtapHidClient::open_hid,
            ));
            match result {
                Ok(client) => {
                    self.client = client;
                    return;
                }
                Err(_) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }
    }
}

/// Serialize `request` into `buf` in CTAPHID `CBOR` payload form
/// (`<cmd_byte> || <cbor_body>`). Returns the total length written.
/// Mirrors `ctap2::Authenticator::from_command`'s parse table.
fn serialize_request(request: &Request, buf: &mut [u8]) -> Result<usize, ctap2::Error> {
    use cbor_smol::cbor_serialize;

    let cmd: u8 = match request {
        Request::MakeCredential(_) => 0x01,
        Request::GetAssertion(_) => 0x02,
        Request::GetNextAssertion => 0x08,
        Request::GetInfo => 0x04,
        Request::ClientPin(_) => 0x06,
        Request::Reset => 0x07,
        Request::CredentialManagement(_) => 0x0A,
        Request::Selection => 0x0B,
        Request::LargeBlobs(_) => 0x0C,
        Request::Config(_) => 0x0D,
        // Vendor commands aren't fixed to a single opcode; the Vendor enum
        // wraps an opcode + opaque body. Pushed through `call_ctap2_raw`
        // by tests that need it; this typed path isn't useful.
        Request::Vendor(_) => return Err(ctap2::Error::Other),
        // `Request` is `#[non_exhaustive]`; future variants land here.
        _ => return Err(ctap2::Error::InvalidCommand),
    };
    buf[0] = cmd;

    let body = match request {
        Request::MakeCredential(req) => cbor_serialize(req, &mut buf[1..]),
        Request::GetAssertion(req) => cbor_serialize(req, &mut buf[1..]),
        Request::ClientPin(req) => cbor_serialize(req, &mut buf[1..]),
        Request::CredentialManagement(req) => cbor_serialize(req, &mut buf[1..]),
        Request::LargeBlobs(req) => cbor_serialize(req, &mut buf[1..]),
        Request::Config(req) => cbor_serialize(req, &mut buf[1..]),
        // No-body variants — the command byte alone is the wire form.
        Request::GetInfo
        | Request::GetNextAssertion
        | Request::Reset
        | Request::Selection
        | Request::Vendor(_) => return Ok(1),
        // `Request` is `#[non_exhaustive]`; future variants land here.
        _ => return Err(ctap2::Error::InvalidCommand),
    }
    .map_err(|e| {
        eprintln!("[transport] CBOR serialize failed: {e:?}");
        ctap2::Error::Other
    })?;

    Ok(1 + body.len())
}

/// Decode a CTAPHID `CBOR` response body into the `Response` variant that
/// matches `request`. Status-byte handling (errors) is done by the caller.
fn deserialize_response(request: &Request, body: &[u8]) -> Result<Response, ctap2::Error> {
    fn de<'de, T: serde::Deserialize<'de>>(body: &'de [u8]) -> Result<T, ctap2::Error> {
        cbor_smol::cbor_deserialize(body).map_err(|e| {
            eprintln!("[transport] CBOR deserialize failed: {e:?} body={body:02X?}");
            ctap2::Error::Other
        })
    }
    // Some CTAP commands legitimately return `status=0x00` with NO CBOR
    // body (set-style ClientPin subcommands, certain CredentialManagement
    // updates, …). For Response types that derive `Default`, an absent
    // body is equivalent to "all optional fields = None".
    fn de_or_default<'de, T: serde::Deserialize<'de> + Default>(
        body: &'de [u8],
    ) -> Result<T, ctap2::Error> {
        if body.is_empty() {
            Ok(T::default())
        } else {
            cbor_smol::cbor_deserialize(body).map_err(|e| {
                eprintln!("[transport] CBOR deserialize failed: {e:?} body={body:02X?}");
                ctap2::Error::Other
            })
        }
    }
    Ok(match request {
        Request::MakeCredential(_) => Response::MakeCredential(de(body)?),
        Request::GetAssertion(_) => Response::GetAssertion(de(body)?),
        Request::GetNextAssertion => Response::GetNextAssertion(de(body)?),
        Request::GetInfo => Response::GetInfo(de(body)?),
        Request::ClientPin(_) => Response::ClientPin(de_or_default(body)?),
        Request::CredentialManagement(_) => Response::CredentialManagement(de_or_default(body)?),
        Request::LargeBlobs(_) => Response::LargeBlobs(de_or_default(body)?),
        Request::Reset => Response::Reset,
        Request::Selection => Response::Selection,
        Request::Config(_) => Response::Config,
        Request::Vendor(_) => Response::Vendor,
        // `Request` is `#[non_exhaustive]`; future variants land here.
        _ => return Err(ctap2::Error::InvalidCommand),
    })
}

pub fn error_from_byte(b: u8) -> ctap2::Error {
    match b {
        0x01 => ctap2::Error::InvalidCommand,
        0x02 => ctap2::Error::InvalidParameter,
        0x03 => ctap2::Error::InvalidLength,
        0x04 => ctap2::Error::InvalidSeq,
        0x05 => ctap2::Error::Timeout,
        0x06 => ctap2::Error::ChannelBusy,
        0x0A => ctap2::Error::LockRequired,
        0x0B => ctap2::Error::InvalidChannel,
        0x11 => ctap2::Error::CborUnexpectedType,
        0x12 => ctap2::Error::InvalidCbor,
        0x14 => ctap2::Error::MissingParameter,
        0x15 => ctap2::Error::LimitExceeded,
        0x16 => ctap2::Error::UnsupportedExtension,
        0x17 => ctap2::Error::FingerprintDatabaseFull,
        0x18 => ctap2::Error::LargeBlobStorageFull,
        0x19 => ctap2::Error::CredentialExcluded,
        0x21 => ctap2::Error::Processing,
        0x22 => ctap2::Error::InvalidCredential,
        0x23 => ctap2::Error::UserActionPending,
        0x24 => ctap2::Error::OperationPending,
        0x25 => ctap2::Error::NoOperations,
        0x26 => ctap2::Error::UnsupportedAlgorithm,
        0x27 => ctap2::Error::OperationDenied,
        0x28 => ctap2::Error::KeyStoreFull,
        0x29 => ctap2::Error::NotBusy,
        0x2A => ctap2::Error::NoOperationPending,
        0x2B => ctap2::Error::UnsupportedOption,
        0x2C => ctap2::Error::InvalidOption,
        0x2D => ctap2::Error::KeepaliveCancel,
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
        0x39 => ctap2::Error::RequestTooLarge,
        0x3A => ctap2::Error::ActionTimeout,
        0x3B => ctap2::Error::UpRequired,
        0x3C => ctap2::Error::UvBlocked,
        0x3D => ctap2::Error::IntegrityFailure,
        0x3E => ctap2::Error::InvalidSubcommand,
        0x3F => ctap2::Error::UvInvalid,
        0x40 => ctap2::Error::UnauthorizedPermission,
        _ => ctap2::Error::Other,
    }
}
