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

    // call with any of `crate::api::request::*`
    pub fn request<'c>(&'c mut self, req: impl Into<Request>) -> FutureResult<'a, 'c> {
        // TODO: handle failure
        self.ep.send.enqueue(req.into()).ok();
        FutureResult::new(self)
    }
}

pub struct FutureResult<'a, 'c> {
    c: &'c mut RawClient<'a>,
}

impl<'a, 'c> FutureResult<'a, 'c> {

    pub fn new(client: &'c mut RawClient<'a>) -> Self {
        Self { c: client }
    }

    pub fn poll(&mut self) -> core::task::Poll<core::result::Result<Reply, Error>> {
        match self.c.ep.recv.dequeue() {
            Some(reply) => {
                #[cfg(all(test, feature = "verbose-tests"))]
                println!("got a reply: {:?}", &reply);
                core::task::Poll::Ready(reply)
            },
            _ => core::task::Poll::Pending
        }
    }
}

// instead of: `let mut future = client.request(request)`
// allows: `let mut future = request.submit(&mut client)`
pub trait SubmitRequest: Into<Request> {
    fn submit<'a, 'c>(self, client: &'c mut RawClient<'a>) -> FutureResult<'a, 'c> {
        client.request(self)
    }
}

impl SubmitRequest for request::GenerateKey {}
impl SubmitRequest for request::GenerateKeypair {}
impl SubmitRequest for request::Sign {}

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
