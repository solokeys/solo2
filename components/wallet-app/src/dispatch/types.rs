//! Interchange types for the wallet HID dispatch.
//!
//! The request and response buffers are **asymmetric** on purpose: a
//! signing request carries a whole transaction (see `REQUEST_SIZE`), while a
//! response is only ever a signature (≤ 72 B), a public key (≤ 65 B), or an
//! exported 32-byte seed. So the request is large and the response is small.

use heapless::Vec;

/// Inbound request buffer size. Holds ONE incoming APDU: a Solana sign (tx +
/// BIP-44 path + header; ≤ ~1.5 KB today) or one Ethereum signTransaction
/// chunk (chunks accumulate into the Authenticator's 4 KB `sign_buf`). Kept small
/// because lpc55 RAM is maxed; a future >2 KB single-APDU tx must be chunked, or
/// this bumped once RAM is freed (e.g. by de-duplicating with `sign_buf`).
pub const REQUEST_SIZE: usize = 2048;
/// Outbound response buffer size. A signature / public key / 32-byte seed
/// all fit easily; 512 is generous margin.
pub const RESPONSE_SIZE: usize = 512;
/// Default size for the generic request `Message` = the request buffer.
pub const DEFAULT_MESSAGE_SIZE: usize = REQUEST_SIZE;

/// Inbound request message buffer. Generic over `N` (default = request size)
/// so the runner can pick the capacity; in practice it's `REQUEST_SIZE`.
pub type Message<const N: usize = DEFAULT_MESSAGE_SIZE> = Vec<u8, N>;
/// Outbound response message buffer — fixed small size, asymmetric with the
/// request.
pub type ResponseMessage = Vec<u8, RESPONSE_SIZE>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NoResponse,
    InvalidLength,
}

pub type AppResult = core::result::Result<(), Error>;

/// Wrapper around `Result<ResponseMessage, Error>` so we can `impl Default`
/// for `interchange::Responder::response_mut` ergonomics. The response side
/// is fixed at `RESPONSE_SIZE` (not generic) — it never needs to be large.
pub struct InterchangeResponse(pub Result<ResponseMessage, Error>);

impl Default for InterchangeResponse {
    fn default() -> Self {
        Self(Ok(ResponseMessage::new()))
    }
}

impl From<Result<ResponseMessage, Error>> for InterchangeResponse {
    fn from(value: Result<ResponseMessage, Error>) -> Self {
        Self(value)
    }
}

impl From<InterchangeResponse> for Result<ResponseMessage, Error> {
    fn from(value: InterchangeResponse) -> Self {
        value.0
    }
}

pub type Channel<const N: usize = DEFAULT_MESSAGE_SIZE> =
    interchange::Channel<Message<N>, InterchangeResponse>;
pub type Requester<'pipe, const N: usize = DEFAULT_MESSAGE_SIZE> =
    interchange::Requester<'pipe, Message<N>, InterchangeResponse>;
pub type Responder<'pipe, const N: usize = DEFAULT_MESSAGE_SIZE> =
    interchange::Responder<'pipe, Message<N>, InterchangeResponse>;
