use core::marker::PhantomData;

use interchange::Requester;

use crate::api::*;
// use crate::config::*;
use crate::error::*;
use crate::pipe::TrussedInterchange;
use crate::types::*;

// to be fair, this is a programmer error,
// and could also just panic
#[derive(Copy, Clone, Debug)]
pub enum ClientError {
    Full,
    Pending,
    DataTooLarge,
}

#[macro_export]
macro_rules! block {
    ($future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $future_result;
        loop {
            match future_result.poll() {
                core::task::Poll::Ready(result) => { break result; },
                core::task::Poll::Pending => {},
            }
        }
    }}
}

#[macro_export]
macro_rules! syscall {
    ($pre_future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $pre_future_result.expect("no client error");
        loop {
            match future_result.poll() {
                core::task::Poll::Ready(result) => { break result.expect("no errors"); },
                core::task::Poll::Pending => {},
            }
        }
    }}
}


pub struct RawClient {
    pub(crate) interchange: Requester<TrussedInterchange>,
    // pending: Option<Discriminant<Request>>,
    pending: Option<u8>,
}

impl RawClient {
    pub fn new(interchange: Requester<TrussedInterchange>) -> Self {
        Self { interchange, pending: None }
    }

    // call with any of `crate::api::request::*`
    pub fn request<'c>(&'c mut self, req: impl Into<Request>)
        -> core::result::Result<RawFutureResult<'c>, ClientError>
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
        self.interchange.request(request).map_err(drop).unwrap();
        Ok(RawFutureResult::new(self))
    }
}

pub struct RawFutureResult<'c> {
    c: &'c mut RawClient,
}

impl<'c> RawFutureResult<'c> {

    pub fn new(client: &'c mut RawClient) -> Self {
        Self { c: client }
    }

    pub fn poll(&mut self)
        -> core::task::Poll<core::result::Result<Reply, Error>>
    {
        match self.c.interchange.take_response() {
            Some(reply) => {
                // #[cfg(all(test, feature = "verbose-tests"))]
                // println!("got a reply: {:?}", &reply);
                match reply {
                    Ok(reply) => {
                        if Some(u8::from(&reply)) == self.c.pending {
                            self.c.pending = None;
                            core::task::Poll::Ready(Ok(reply))
                        } else  {
                            // #[cfg(all(test, feature = "verbose-tests"))]
                            info!("got: {:?}, expected: {:?}", Some(u8::from(&reply)), self.c.pending).ok();
                            core::task::Poll::Ready(Err(Error::InternalError))
                        }
                    }
                    Err(error) => {
                        self.c.pending = None;
                        core::task::Poll::Ready(Err(error))
                    }
                }

            },
            None => core::task::Poll::Pending
        }
    }
}

// instead of: `let mut future = client.request(request)`
// allows: `let mut future = request.submit(&mut client)`
pub trait SubmitRequest: Into<Request> {
    fn submit<'c>(self, client: &'c mut RawClient)
        -> Result<RawFutureResult<'c>, ClientError>
    {
        client.request(self)
    }
}

impl SubmitRequest for request::GenerateKey {}
// impl SubmitRequest for request::GenerateKeypair {}
impl SubmitRequest for request::Sign {}
impl SubmitRequest for request::Verify {}

pub struct FutureResult<'c, T> {
    f: RawFutureResult<'c>,
    __: PhantomData<T>,
}

impl<'c, T> FutureResult<'c, T>
where
    T: From<crate::api::Reply>
{
    pub fn new<S: crate::pipe::Syscall>(client: &'c mut Client<S>) -> Self {
        Self { f: RawFutureResult::new(&mut client.raw), __: PhantomData }
    }

    pub fn poll(&mut self)
        -> core::task::Poll<core::result::Result<T, Error>>
    {
        use core::task::Poll::{Pending, Ready};
        match self.f.poll() {
            // Ready(Ok(reply)) => {
            //     println!("my first match arm");
            //     match T::try_from(reply) {
            //         Ok(reply2) => {
            //             println!("my second match arm");
            //             Ready(Ok(reply2))
            //         },
            //         Err(_) => {
            //             println!("not my second match arm");
            //             Ready(Err(Error::ImplementationError))
            //         }
            //     }
            // }
            Ready(Ok(reply)) => Ready(Ok(T::from(reply))),
            Ready(Err(error)) => Ready(Err(error)),
            Pending => Pending
        }
    }
}

#[derive(Default)]
pub struct TrussedSyscall {}

impl crate::pipe::Syscall for TrussedSyscall {
    #[inline]
    fn syscall(&mut self) {
        rtic::pend(lpc55_hal::raw::Interrupt::OS_EVENT);
    }
}

pub struct Client<Syscall: crate::pipe::Syscall = TrussedSyscall> {
    raw: RawClient,
    syscall: Syscall,
}

impl<Syscall: crate::pipe::Syscall> From<(RawClient, Syscall)> for Client<Syscall> {
    fn from(input: (RawClient, Syscall)) -> Self {
        Self { raw: input.0, syscall: input.1 }
    }
}

impl<Syscall: crate::pipe::Syscall> Client<Syscall> {
    pub fn new(interchange: Requester<TrussedInterchange>, syscall: Syscall) -> Self {
        Self { raw: RawClient::new(interchange), syscall }
    }


    pub fn agree<'c>(
        &'c mut self, mechanism: Mechanism,
        private_key: ObjectHandle, public_key: ObjectHandle,
        attributes: StorageAttributes,
        )
        -> core::result::Result<FutureResult<'c, reply::Agree>, ClientError>
    {
        self.raw.request(request::Agree {
            mechanism,
            private_key,
            public_key,
            attributes,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn agree_p256<'c>(&'c mut self, private_key: &ObjectHandle, public_key: &ObjectHandle, persistence: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::Agree>, ClientError>
    {
        self.agree(
            Mechanism::P256,
            private_key.clone(),
            public_key.clone(),
            StorageAttributes::new().set_persistence(persistence),
        )
    }

    pub fn derive_key<'c>(&'c mut self, mechanism: Mechanism, base_key: ObjectHandle, attributes: StorageAttributes)
        -> core::result::Result<FutureResult<'c, reply::DeriveKey>, ClientError>
    {
        self.raw.request(request::DeriveKey {
            mechanism,
            base_key,
            attributes,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

          // - mechanism: Mechanism
          // - key: ObjectHandle
          // - message: Message
          // - associated_data: ShortData
    pub fn encrypt<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle,
                       message: &[u8], associated_data: &[u8], nonce: Option<ShortData>)
        -> core::result::Result<FutureResult<'c, reply::Encrypt>, ClientError>
    {
        let message = Message::from_slice(message).map_err(|_| ClientError::DataTooLarge)?;
        let associated_data = ShortData::from_slice(associated_data).map_err(|_| ClientError::DataTooLarge)?;
        self.raw.request(request::Encrypt { mechanism, key, message, associated_data, nonce })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

          // - mechanism: Mechanism
          // - key: ObjectHandle
          // - message: Message
          // - associated_data: ShortData
          // - nonce: ShortData
          // - tag: ShortData
    pub fn decrypt<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle,
                       message: &[u8], associated_data: &[u8],
                       nonce: &[u8], tag: &[u8],
                       )
        -> core::result::Result<FutureResult<'c, reply::Decrypt>, ClientError>
    {
        let message = Message::from_slice(message).map_err(|_| ClientError::DataTooLarge)?;
        let associated_data = Message::from_slice(associated_data).map_err(|_| ClientError::DataTooLarge)?;
        let nonce = ShortData::from_slice(nonce).map_err(|_| ClientError::DataTooLarge)?;
        let tag = ShortData::from_slice(tag).map_err(|_| ClientError::DataTooLarge)?;
        self.raw.request(request::Decrypt { mechanism, key, message, associated_data, nonce, tag })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

          // - mechanism: Mechanism
          // - serialized_key: Message
          // - format: KeySerialization
          // - attributes: StorageAttributes
    pub fn deserialize_key<'c>(&'c mut self, mechanism: Mechanism, serialized_key: Message,
                               format: KeySerialization, attributes: StorageAttributes)
        -> core::result::Result<FutureResult<'c, reply::DeserializeKey>, ClientError>
    {
        self.raw.request(request::DeserializeKey {
            mechanism, serialized_key, format, attributes
        } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn delete<'c>(
        &'c mut self,
        // mechanism: Mechanism,
        key: ObjectHandle,
    )
        -> core::result::Result<FutureResult<'c, reply::Delete>, ClientError>
    {
        self.raw.request(request::Delete {
            key,
            // mechanism,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn debug_dump_store<'c>(
        &'c mut self,
    )
        -> core::result::Result<FutureResult<'c, reply::DebugDumpStore>, ClientError>
    {
        self.raw.request(request::DebugDumpStore {})?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn exists<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
    )
        -> core::result::Result<FutureResult<'c, reply::Exists>, ClientError>
    {
        self.raw.request(request::Exists {
            key,
            mechanism,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn generate_key<'c>(&'c mut self, mechanism: Mechanism, attributes: StorageAttributes)
        -> core::result::Result<FutureResult<'c, reply::GenerateKey>, ClientError>
    {
        self.raw.request(request::GenerateKey {
            mechanism,
            attributes,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn read_dir_first<'c>(
        &'c mut self,
        location: StorageLocation,
        dir: PathBuf,
        not_before_filename: Option<PathBuf>,
    )
        -> core::result::Result<FutureResult<'c, reply::ReadDirFirst>, ClientError>
    {
        self.raw.request(request::ReadDirFirst { location, dir, not_before_filename } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn read_dir_next<'c>(
        &'c mut self,
    )
        -> core::result::Result<FutureResult<'c, reply::ReadDirNext>, ClientError>
    {
        self.raw.request(request::ReadDirNext {} )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn read_dir_files_first<'c>(
        &'c mut self,
        location: StorageLocation,
        dir: PathBuf,
        user_attribute: Option<UserAttribute>,
    )
        -> core::result::Result<FutureResult<'c, reply::ReadDirFilesFirst>, ClientError>
    {
        self.raw.request(request::ReadDirFilesFirst { dir, location, user_attribute } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn read_dir_files_next<'c>(
        &'c mut self,
    )
        -> core::result::Result<FutureResult<'c, reply::ReadDirFilesNext>, ClientError>
    {
        self.raw.request(request::ReadDirFilesNext {} )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn remove_dir<'c>(&'c mut self, location: StorageLocation, path: PathBuf)
        -> core::result::Result<FutureResult<'c, reply::RemoveFile>, ClientError>
    {
        self.raw.request(request::RemoveFile { location, path } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn remove_file<'c>(&'c mut self, location: StorageLocation, path: PathBuf)
        -> core::result::Result<FutureResult<'c, reply::RemoveFile>, ClientError>
    {
        self.raw.request(request::RemoveFile { location, path } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn read_file<'c>(&'c mut self, location: StorageLocation, path: PathBuf)
        -> core::result::Result<FutureResult<'c, reply::ReadFile>, ClientError>
    {
        self.raw.request(request::ReadFile { location, path } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn locate_file<'c>(&'c mut self, location: StorageLocation,
                           dir: Option<PathBuf>,
                           filename: PathBuf,
                           )
        -> core::result::Result<FutureResult<'c, reply::LocateFile>, ClientError>
    {
        self.raw.request(request::LocateFile { location, dir, filename } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn write_file<'c>(
        &'c mut self,
        location: StorageLocation,
        path: PathBuf,
        data: Message,
        user_attribute: Option<UserAttribute>,
        )
        -> core::result::Result<FutureResult<'c, reply::WriteFile>, ClientError>
    {
        self.raw.request(request::WriteFile {
            location, path, data,
            user_attribute,
        } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }
          // - mechanism: Mechanism
          // - key: ObjectHandle
          // - format: KeySerialization

    pub fn serialize_key<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle, format: KeySerialization)
        -> core::result::Result<FutureResult<'c, reply::SerializeKey>, ClientError>
    {
        self.raw.request(request::SerializeKey {
            key,
            mechanism,
            format,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn sign<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
        data: &[u8],
        format: SignatureSerialization,
    )
        -> core::result::Result<FutureResult<'c, reply::Sign>, ClientError>
    {
        self.raw.request(request::Sign {
            key,
            mechanism,
            message: ByteBuf::from_slice(data).map_err(|_| ClientError::DataTooLarge)?,
            format,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn verify<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
        message: &[u8],
        signature: &[u8],
        format: SignatureSerialization,
    )
        -> core::result::Result<FutureResult<'c, reply::Verify>, ClientError>
    {
        self.raw.request(request::Verify {
            mechanism,
            key,
            message: Message::from_slice(&message).expect("all good"),
            signature: Signature::from_slice(&signature).expect("all good"),
            format,
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }


    pub fn random_bytes<'c>(&'c mut self, count: usize)
        -> core::result::Result<FutureResult<'c, reply::RandomByteBuf>, ClientError>
    {
        self.raw.request(request::RandomByteBuf { count } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn hash<'c>(&'c mut self, mechanism: Mechanism, message: Message)
        -> core::result::Result<FutureResult<'c, reply::Hash>, ClientError>
    {
        self.raw.request(request::Hash { mechanism, message } )?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn hash_sha256<'c>(&'c mut self, message: &[u8])
        -> core::result::Result<FutureResult<'c, reply::Hash>, ClientError>
    {
        self.hash(Mechanism::Sha256, Message::from_slice(message).map_err(|_| ClientError::DataTooLarge)?)
    }

    pub fn decrypt_chacha8poly1305<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], associated_data: &[u8],
                                       nonce: &[u8], tag: &[u8])
        -> core::result::Result<FutureResult<'c, reply::Decrypt>, ClientError>
    {
        self.decrypt(Mechanism::Chacha8Poly1305, key.clone(), message, associated_data, nonce, tag)
    }

    pub fn decrypt_aes256cbc<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> core::result::Result<FutureResult<'c, reply::Decrypt>, ClientError>
    {
        self.decrypt(
            Mechanism::Aes256Cbc, key.clone(), message, &[], &[], &[],
        )
    }

    pub fn encrypt_chacha8poly1305<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], associated_data: &[u8],
                                       nonce: Option<&[u8; 12]>)
        -> core::result::Result<FutureResult<'c, reply::Encrypt>, ClientError>
    {
        self.encrypt(Mechanism::Chacha8Poly1305, key.clone(), message, associated_data,
            nonce.and_then(|nonce| ShortData::from_slice(nonce).ok()))
    }

    pub fn decrypt_tdes<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> core::result::Result<FutureResult<'c, reply::Decrypt>, ClientError>
    {
        self.decrypt(Mechanism::Tdes, key.clone(), message, &[], &[], &[])
    }

    pub fn encrypt_tdes<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> core::result::Result<FutureResult<'c, reply::Encrypt>, ClientError>
    {
        self.encrypt(Mechanism::Tdes, key.clone(), message, &[], None)
    }

    pub fn unsafe_inject_totp_key<'c>(&'c mut self, raw_key: &[u8; 20], persistence: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::UnsafeInjectKey>, ClientError>
    {
        cortex_m_semihosting::hprintln!("{}B: raw key: {:X?}", raw_key.len(), raw_key).ok();
        self.raw.request(request::UnsafeInjectKey {
            mechanism: Mechanism::Totp,
            raw_key: ShortData::from_slice(raw_key).unwrap(),
            attributes: StorageAttributes::new().set_persistence(persistence),
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn unsafe_inject_tdes_key<'c>(&'c mut self, raw_key: &[u8; 24], persistence: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::UnsafeInjectKey>, ClientError>
    {
        self.raw.request(request::UnsafeInjectKey {
            mechanism: Mechanism::Tdes,
            raw_key: ShortData::from_slice(raw_key).unwrap(),
            attributes: StorageAttributes::new().set_persistence(persistence),
        })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn generate_chacha8poly1305_key<'c>(&'c mut self, persistence: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::GenerateKey>, ClientError>
    {
        self.generate_key(Mechanism::Chacha8Poly1305, StorageAttributes::new().set_persistence(persistence))
    }

    pub fn generate_ed25519_private_key<'c>(&'c mut self, persistence: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::GenerateKey>, ClientError>
    {
        self.generate_key(Mechanism::Ed25519, StorageAttributes::new().set_persistence(persistence))
    }

    pub fn generate_hmacsha256_key<'c>(&'c mut self, persistence: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::GenerateKey>, ClientError>
    {
        self.generate_key(Mechanism::HmacSha256, StorageAttributes::new().set_persistence(persistence))
    }

    pub fn derive_ed25519_public_key<'c>(&'c mut self, private_key: &ObjectHandle, persistence: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::DeriveKey>, ClientError>
    {
        self.derive_key(Mechanism::Ed25519, private_key.clone(), StorageAttributes::new().set_persistence(persistence))
    }

    pub fn generate_p256_private_key<'c>(&'c mut self, persistence: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::GenerateKey>, ClientError>
    {
        self.generate_key(Mechanism::P256, StorageAttributes::new().set_persistence(persistence))
    }

    pub fn derive_p256_public_key<'c>(&'c mut self, private_key: &ObjectHandle, persistence: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::DeriveKey>, ClientError>
    {
        self.derive_key(Mechanism::P256, private_key.clone(), StorageAttributes::new().set_persistence(persistence))
    }

    pub fn sign_ed25519<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> core::result::Result<FutureResult<'c, reply::Sign>, ClientError>
    {
        self.sign(Mechanism::Ed25519, key.clone(), message, SignatureSerialization::Raw)
    }

    pub fn sign_hmacsha256<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> core::result::Result<FutureResult<'c, reply::Sign>, ClientError>
    {
        self.sign(Mechanism::HmacSha256, key.clone(), message, SignatureSerialization::Raw)
    }

    // generally, don't offer multiple versions of a mechanism, if possible.
    // try using the simplest when given the choice.
    // hashing is something users can do themselves hopefully :)
    //
    // on the other hand: if users need sha256, then if the service runs in secure trustzone
    // domain, we'll maybe need two copies of the sha2 code
    pub fn sign_p256<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], format: SignatureSerialization)
        -> core::result::Result<FutureResult<'c, reply::Sign>, ClientError>
    {
        self.sign(Mechanism::P256, key.clone(), message, format)
    }

    pub fn sign_totp<'c>(&'c mut self, key: &ObjectHandle, timestamp: u64)
        -> core::result::Result<FutureResult<'c, reply::Sign>, ClientError>
    {
        self.sign(Mechanism::Totp, key.clone(),
            &timestamp.to_le_bytes().as_ref(),
            SignatureSerialization::Raw,
        )
    }

          // - mechanism: Mechanism
          // - wrapping_key: ObjectHandle
          // - wrapped_key: Message
          // - associated_data: Message
    pub fn unwrap_key<'c>(&'c mut self, mechanism: Mechanism, wrapping_key: ObjectHandle, wrapped_key: Message,
                       associated_data: &[u8], attributes: StorageAttributes)
        -> core::result::Result<FutureResult<'c, reply::UnwrapKey>, ClientError>
    {
        let associated_data = Message::from_slice(associated_data).map_err(|_| ClientError::DataTooLarge)?;
        self.raw.request(request::UnwrapKey { mechanism, wrapping_key, wrapped_key, associated_data, attributes })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn unwrap_key_chacha8poly1305<'c>(&'c mut self, wrapping_key: &ObjectHandle, wrapped_key: &Message,
                       associated_data: &[u8], location: StorageLocation)
        -> core::result::Result<FutureResult<'c, reply::UnwrapKey>, ClientError>
    {
        self.unwrap_key(Mechanism::Chacha8Poly1305, wrapping_key.clone(), wrapped_key.clone(), associated_data,
                         StorageAttributes::new().set_persistence(location))
    }

    pub fn verify_ed25519<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], signature: &[u8])
        -> core::result::Result<FutureResult<'c, reply::Verify>, ClientError>
    {
        self.verify(Mechanism::Ed25519, key.clone(), message, signature, SignatureSerialization::Raw)
    }

    pub fn verify_p256<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], signature: &[u8])
        -> core::result::Result<FutureResult<'c, reply::Verify>, ClientError>
    {
        self.verify(Mechanism::P256, key.clone(), message, signature, SignatureSerialization::Raw)
    }

          // - mechanism: Mechanism
          // - wrapping_key: ObjectHandle
          // - key: ObjectHandle
          // - associated_data: Message
    pub fn wrap_key<'c>(&'c mut self, mechanism: Mechanism, wrapping_key: ObjectHandle, key: ObjectHandle,
                       associated_data: &[u8])
        -> core::result::Result<FutureResult<'c, reply::WrapKey>, ClientError>
    {
        let associated_data = Message::from_slice(associated_data).map_err(|_| ClientError::DataTooLarge)?;
        self.raw.request(request::WrapKey { mechanism, wrapping_key, key, associated_data })?;
        self.syscall.syscall();
        Ok(FutureResult::new(self))
    }

    pub fn wrap_key_chacha8poly1305<'c>(&'c mut self, wrapping_key: &ObjectHandle, key: &ObjectHandle,
                       associated_data: &[u8])
        -> core::result::Result<FutureResult<'c, reply::WrapKey>, ClientError>
    {
        self.wrap_key(Mechanism::Chacha8Poly1305, wrapping_key.clone(), key.clone(), associated_data)
    }

    pub fn wrap_key_aes256cbc<'c>(&'c mut self, wrapping_key: &ObjectHandle, key: &ObjectHandle)
        -> core::result::Result<FutureResult<'c, reply::WrapKey>, ClientError>
    {
        self.wrap_key(Mechanism::Aes256Cbc, wrapping_key.clone(), key.clone(), &[])
    }


}
