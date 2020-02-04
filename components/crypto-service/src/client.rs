use crate::api::*;
use crate::config::*;
use crate::error::*;
use crate::types::*;

pub use crate::pipe::ClientEndpoint;

pub struct RawClient<'a> {
    pub(crate) ep: ClientEndpoint<'a>,
}

impl<'a> RawClient<'a> {
    pub fn new(ep: ClientEndpoint<'a>) -> Self {
        Self { ep }
    }

    pub fn request<'c>(&'c mut self, req: Request) -> FutureResult<'a, 'c> {
        // TODO: handle failure
        self.ep.send.enqueue(req).ok();
        FutureResult::new(self)
    }
}

pub struct Client {
}

impl Client {
    pub fn sign(&mut self, key_handle: KeyHandle, mechanism: Mechanism, data: &[u8])
        -> core::task::Poll<Signature>
    {
        todo!();
    }

    // hmm this function signature
    // what i want is to temporarily borrow the receiving end of the
    // pipe to Crypty, and release it once the signature result arrives.
    //
    // Also note recent discussion on CFRG: do NOT pass in public key
    // separately (users can mix up, implementation detail of Crypty
    // whether to cache/store the public key with the private key as key
    // pair, or calculate public from private key on the fly).
    // pub fn sign_ed25519(&mut self, private_key: KeyHandle, data: &[u8])
    //     -> FutureResult<Signature>
    // {
    //     todo!();
    // }

    // generally, don't offer multiple versions of a mechanism, if possible.
    // try using the simplest when given the choice.
    // hashing is something users can do themselves hopefully :)
    // pub fn sign_p256_prehashed(&mut self, private_key: KeyHandle, data: &[u8])
    //     -> core::task::Poll<Signature>
    // {
    //     todo!();
    // }

}
