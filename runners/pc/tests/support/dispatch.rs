//! In-process CTAPHID dispatch transport.
//!
//! **STUBBED.** Relied on `ctap_types::Request` CBOR round-trip, which no
//! longer works in 0.5 (Request<'_> is deserialize-only from out-of-crate,
//! and no `ctaphid_dispatch::app::Message` type alias exists in 0.4). Not used
//! by `fido2.rs`'s `with_authenticator!` macro, so kept here as a stub for
//! potential future use if we add a raw-CBOR serializer path.
#![allow(dead_code)]

use ctap_types::ctap2::{self, Request, Response};
use ctaphid_dispatch::app::App;

use super::transport::TestAuthenticator;

pub struct DispatchTransport<'a, A: App<'a>> {
    _app: &'a mut A,
}

impl<'a, A: App<'a>> DispatchTransport<'a, A> {
    pub fn new(app: &'a mut A) -> Self {
        Self { _app: app }
    }
}

impl<'a, A: App<'a>> TestAuthenticator for DispatchTransport<'a, A> {
    fn call_ctap2(&mut self, _request: &Request) -> Result<Response, ctap2::Error> {
        unimplemented!("DispatchTransport: stubbed; see support/dispatch.rs header")
    }

    fn call_ctap2_raw(
        &mut self,
        _command: u8,
        _payload: &[u8],
    ) -> Result<(u8, Vec<u8>), ctap2::Error> {
        unimplemented!("DispatchTransport: stubbed")
    }
}
