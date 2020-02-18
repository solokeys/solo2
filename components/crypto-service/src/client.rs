use core::marker::PhantomData;

use crate::api::*;
// use crate::config::*;
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
    // pending: Option<Discriminant<Request>>,
    pending: Option<u8>,
}

impl<'a> RawClient<'a> {
    pub fn new(ep: ClientEndpoint<'a>) -> Self {
        Self { ep, pending: None }
    }

    // call with any of `crate::api::request::*`
    pub fn request<'c>(&'c mut self, req: impl Into<Request>)
        -> core::result::Result<RawFutureResult<'a, 'c>, ClientError>
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
        Ok(RawFutureResult::new(self))
    }
}

pub struct RawFutureResult<'a, 'c> {
    c: &'c mut RawClient<'a>,
}

impl<'a, 'c> RawFutureResult<'a, 'c> {

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
        -> Result<RawFutureResult<'a, 'c>, ClientError>
    {
        client.request(self)
    }
}

impl SubmitRequest for request::GenerateKey {}
// impl SubmitRequest for request::GenerateKeypair {}
impl SubmitRequest for request::Sign {}
impl SubmitRequest for request::Verify {}

pub struct FutureResult<'a, 'c, T> {
    f: RawFutureResult<'a, 'c>,
    __: PhantomData<T>,
}

impl<'a, 'c, T> FutureResult<'a, 'c, T>
where
    T: From<crate::api::Reply>
{
    pub fn new<S: crate::pipe::Syscall>(client: &'c mut Client<'a, S>) -> Self {
        Self { f: RawFutureResult::new(&mut client.raw), __: PhantomData }
    }

    pub fn poll(&mut self)
        -> core::task::Poll<core::result::Result<T, Error>>
    {
        use core::task::Poll::{Pending, Ready};
        use core::convert::TryFrom;
        match self.f.poll() {
            Ready(Ok(reply)) => {
                match T::try_from(reply) {
                    Ok(reply2) => Ready(Ok(reply2)),
                    Err(_) => Ready(Err(Error::ImplementationError)),
                }
                // Ready(Ok(reply.into()))
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


    pub fn derive_key<'c>(&'c mut self, mechanism: Mechanism, base_key: ObjectHandle)
        -> core::result::Result<FutureResult<'a, 'c, reply::DeriveKey>, ClientError>
    {
        self.raw.request(request::DeriveKey {
            mechanism,
            base_key,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn generate_key<'c>(&'c mut self, mechanism: Mechanism)
        -> core::result::Result<FutureResult<'a, 'c, reply::GenerateKey>, ClientError>
    {
        self.raw.request(request::GenerateKey {
            mechanism,
            attributes: KeyAttributes::default(),
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn sign<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle, data: &[u8])
        -> core::result::Result<FutureResult<'a, 'c, reply::Sign>, ClientError>
    {
        self.raw.request(request::Sign {
            key,
            mechanism,
            message: Bytes::try_from_slice(data).map_err(|_| ClientError::SignDataTooLarge)?,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn verify<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
        message: &[u8],
        signature: &[u8]
    )
        -> core::result::Result<FutureResult<'a, 'c, reply::Verify>, ClientError>
    {
        self.raw.request(request::Verify {
            mechanism,
            key,
            message: Message::try_from_slice(&message).expect("all good"),
            signature: Signature::try_from_slice(&signature).expect("all good"),
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }


    pub fn generate_ed25519_private_key<'c>(&'c mut self)
        -> core::result::Result<FutureResult<'a, 'c, reply::GenerateKey>, ClientError>
    {
        self.generate_key(Mechanism::Ed25519)
    }

    pub fn derive_ed25519_public_key<'c>(&'c mut self, private_key: &ObjectHandle)
        -> core::result::Result<FutureResult<'a, 'c, reply::DeriveKey>, ClientError>
    {
        self.derive_key(Mechanism::Ed25519, private_key.clone())
    }

    pub fn generate_p256_private_key<'c>(&'c mut self)
        -> core::result::Result<FutureResult<'a, 'c, reply::GenerateKey>, ClientError>
    {
        self.generate_key(Mechanism::P256)
    }

    pub fn derive_p256_public_key<'c>(&'c mut self, private_key: &ObjectHandle)
        -> core::result::Result<FutureResult<'a, 'c, reply::DeriveKey>, ClientError>
    {
        self.derive_key(Mechanism::P256, private_key.clone())
    }

    pub fn sign_ed25519<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> core::result::Result<FutureResult<'a, 'c, reply::Sign>, ClientError>
    {
        self.sign(Mechanism::Ed25519, *key, message)
    }

    // generally, don't offer multiple versions of a mechanism, if possible.
    // try using the simplest when given the choice.
    // hashing is something users can do themselves hopefully :)
    //
    // on the other hand: if users need sha256, then if the service runs in secure trustzone
    // domain, we'll maybe need two copies of the sha2 code
    pub fn sign_p256<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> core::result::Result<FutureResult<'a, 'c, reply::Sign>, ClientError>
    {
        self.sign(Mechanism::P256, *key, message)
    }


    pub fn verify_ed25519<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], signature: &[u8])
        -> core::result::Result<FutureResult<'a, 'c, reply::Verify>, ClientError>
    {
        self.verify(Mechanism::Ed25519, *key, message, signature)
    }

    pub fn verify_p256<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], signature: &[u8])
        -> core::result::Result<FutureResult<'a, 'c, reply::Verify>, ClientError>
    {
        self.verify(Mechanism::P256, *key, message, signature)
    }

}
