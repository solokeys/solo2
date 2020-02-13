use core::marker::PhantomData;

use crate::api::*;
use crate::config::*;
use crate::error::*;
use crate::types::*;

pub use crate::pipe::ClientEndpoint;

// to be fair, this is a programmer error,
// and could also just panic
#[derive(Copy, Clone, Debug)]
pub enum ClientError {
    Full,
    Pending,
    SignDataTooLarge,
}

pub struct RawClient<'a> {
    pub(crate) ep: ClientEndpoint<'a>,
    pending: Option<u8>,
}

impl<'a> RawClient<'a> {
    pub fn new(ep: ClientEndpoint<'a>) -> Self {
        Self { ep, pending: None }
    }

    // call with any of `crate::api::request::*`
    pub fn request<'c>(&'c mut self, req: impl Into<Request>)
        -> core::result::Result<FutureResult<'a, 'c>, ClientError>
    {
        // TODO: handle failure
        // TODO: fail on pending (non-canceled) request)
        if self.pending.is_some() {
            return Err(ClientError::Pending);
        }
        // since no pending, also queue empty
        // if !self.ready() {
        //     return Err(ClientError::Fulle);
        // }
        // in particular, can unwrap
        let request = req.into();
        self.pending = Some(u8::from(&request));
        self.ep.send.enqueue(request).map_err(drop).unwrap();
        Ok(FutureResult::new(self))
    }
}

pub struct FutureResult<'a, 'c> {
    c: &'c mut RawClient<'a>,
}

impl<'a, 'c> FutureResult<'a, 'c> {

    pub fn new(client: &'c mut RawClient<'a>) -> Self {
        Self { c: client }
    }

    pub fn poll(&mut self)
        -> core::task::Poll<core::result::Result<Reply, Error>>
    {
        match self.c.ep.recv.dequeue() {
            Some(reply) => {
                #[cfg(all(test, feature = "verbose-tests"))]
                println!("got a reply: {:?}", &reply);
                match reply {
                    Ok(reply) => {
                        if Some(u8::from(&reply)) == self.c.pending {
                            self.c.pending = None;
                            core::task::Poll::Ready(Ok(reply))
                        } else  {
                            #[cfg(all(test, feature = "verbose-tests"))]
                            println!("got: {:?}, expected: {:?}", Some(u8::from(&reply)), self.c.pending);
                            core::task::Poll::Ready(Err(Error::InternalError))
                        }
                    }
                    Err(error) => core::task::Poll::Ready(Err(error)),
                }

            },
            None => core::task::Poll::Pending
        }
    }
}

// instead of: `let mut future = client.request(request)`
// allows: `let mut future = request.submit(&mut client)`
pub trait SubmitRequest: Into<Request> {
    fn submit<'a, 'c>(self, client: &'c mut RawClient<'a>)
        -> Result<FutureResult<'a, 'c>, ClientError>
    {
        client.request(self)
    }
}

impl SubmitRequest for request::GenerateKey {}
impl SubmitRequest for request::GenerateKeypair {}
impl SubmitRequest for request::Sign {}

pub struct NoFuture<'a, 'c, T> {
    f: FutureResult<'a, 'c>,
    __: PhantomData<T>,
}

impl<'a, 'c, T> NoFuture<'a, 'c, T> {

    pub fn new<S: crate::pipe::Syscall>(client: &'c mut Client<'a, S>) -> Self {
        Self { f: FutureResult::new(&mut client.raw), __: PhantomData }
    }
}

impl<'a, 'c> NoFuture<'a, 'c, request::GenerateKeypair> {
    pub fn poll(&mut self)
        -> core::task::Poll<core::result::Result<reply::GenerateKeypair, Error>>
    {
        use core::task::Poll::{Pending, Ready};
        match self.f.poll() {
            Ready(Ok(reply)) => {
                Ready(Ok(reply.into()))
            }
            Ready(Err(error)) => {
                Ready(Err(error))
            }
            Pending => Pending
        }
    }
}

impl<'a, 'c> NoFuture<'a, 'c, request::Sign> {
    pub fn poll(&mut self)
        -> core::task::Poll<core::result::Result<reply::Sign, Error>>
    {
        use core::task::Poll::{Pending, Ready};
        match self.f.poll() {
            Ready(Ok(reply)) => {
                Ready(Ok(reply.into()))
            }
            Ready(Err(error)) => {
                Ready(Err(error))
            }
            Pending => Pending
        }
    }
}

pub struct Client<'a, Syscall: crate::pipe::Syscall> {
    raw: RawClient<'a>,
    syscall: Syscall,
}

impl<'a, Syscall: crate::pipe::Syscall> Client<'a, Syscall> {
    pub fn new(ep: ClientEndpoint<'a>, syscall: Syscall) -> Self {
        Self { raw: RawClient::new(ep), syscall }
    }

    pub fn sign<'c>(&'c mut self, key_handle: ObjectHandle, mechanism: Mechanism, data: &[u8])
        -> core::result::Result<NoFuture<'a, 'c, request::Sign>, ClientError>
    {
        // pub fn request<'c>(&'c mut self, req: impl Into<Request>)
        //     -> core::result::Result<FutureResult<'a, 'c>, ClientError>
        self.raw.request(request::Sign {
            key_handle,
            mechanism,
            message: Bytes::try_from_slice(data).map_err(|_| ClientError::SignDataTooLarge)?,
        })?;
        self.syscall.syscall();
        Ok(NoFuture::new(self))
    }

    pub fn generate_ed25519_keypair<'c>(&'c mut self)
        -> core::result::Result<NoFuture<'a, 'c, request::GenerateKeypair>, ClientError>
    {
        // pub fn request<'c>(&'c mut self, req: impl Into<Request>)
        //     -> core::result::Result<FutureResult<'a, 'c>, ClientError>
        self.raw.request(request::GenerateKeypair {
            mechanism: Mechanism::Ed25519,
            key_attributes: KeyAttributes::default(),
        })?;
        self.syscall.syscall();
        Ok(NoFuture::new(self))
    }

    pub fn sign_ed25519<'c>(&'c mut self, keypair_handle: &ObjectHandle, message: &[u8])
        -> core::result::Result<NoFuture<'a, 'c, request::Sign>, ClientError>
    {
        // pub fn request<'c>(&'c mut self, req: impl Into<Request>)
        //     -> core::result::Result<FutureResult<'a, 'c>, ClientError>
        self.raw.request(request::Sign {
            key_handle: keypair_handle.clone(),
            mechanism: Mechanism::Ed25519,
            message: Message::try_from_slice(&message).expect("all good"),
        })?;
        self.syscall.syscall();
        Ok(NoFuture::new(self))
    }

    // hmm this function signature
    // what i want is to temporarily borrow the receiving end of the
    // pipe to Crypty, and release it once the signature result arrives.
    //
    // Also note recent discussion on CFRG: do NOT pass in public key
    // separately (users can mix up, implementation detail of Crypty
    // whether to cache/store the public key with the private key as key
    // pair, or calculate public from private key on the fly).
    // pub fn sign_ed25519(&mut self, private_key: ObjectHandle, data: &[u8])
    //     -> FutureResult<Signature>
    // {
    //     todo!();
    // }

    // generally, don't offer multiple versions of a mechanism, if possible.
    // try using the simplest when given the choice.
    // hashing is something users can do themselves hopefully :)
    // pub fn sign_p256_prehashed(&mut self, private_key: ObjectHandle, data: &[u8])
    //     -> core::task::Poll<Signature>
    // {
    //     todo!();
    // }

}
