use core::marker::PhantomData;

use interchange::Requester;

use crate::api::*;
use crate::error::*;
use crate::pipe::TrussedInterchange;
use crate::types::*;

pub use crate::platform::Syscall;

// to be fair, this is a programmer error,
// and could also just panic
#[derive(Copy, Clone, Debug)]
pub enum ClientError {
    Full,
    Pending,
    DataTooLarge,
}

pub type ClientResult<'c, T, C> = core::result::Result<FutureResult<'c, T, C>, ClientError>;

/// Trussed Client interface that Trussed apps can rely on.
pub trait Client {
    fn poll(&mut self) -> core::task::Poll<core::result::Result<Reply, Error>>;

    // call with any of `crate::api::request::*`
    // fn request<'c>(&'c mut self, req: impl Into<Request>)
        // -> core::result::Result<RawFutureResult<'c, Self>, ClientError>;

    fn agree<'c>(
        &'c mut self, mechanism: Mechanism,
        private_key: ObjectHandle, public_key: ObjectHandle,
        attributes: StorageAttributes,
        )
        -> ClientResult<'c, reply::Agree, Self>;

    fn agree_p256<'c>(&'c mut self, private_key: &ObjectHandle, public_key: &ObjectHandle, persistence: StorageLocation)
        -> ClientResult<'c, reply::Agree, Self>
    {
        self.agree(
            Mechanism::P256,
            private_key.clone(),
            public_key.clone(),
            StorageAttributes::new().set_persistence(persistence),
        )
    }

    fn derive_key<'c>(&'c mut self, mechanism: Mechanism, base_key: ObjectHandle, attributes: StorageAttributes)
        -> ClientResult<'c, reply::DeriveKey, Self>;


    fn encrypt<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle,
                       message: &[u8], associated_data: &[u8], nonce: Option<ShortData>)
        -> ClientResult<'c, reply::Encrypt, Self>;


          // - mechanism: Mechanism
          // - key: ObjectHandle
          // - message: Message
          // - associated_data: ShortData
          // - nonce: ShortData
          // - tag: ShortData
    fn decrypt<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle,
                       message: &[u8], associated_data: &[u8],
                       nonce: &[u8], tag: &[u8],
                       )
        -> ClientResult<'c, reply::Decrypt, Self>;


          // - mechanism: Mechanism
          // - serialized_key: Message
          // - format: KeySerialization
          // - attributes: StorageAttributes
    fn deserialize_key<'c>(&'c mut self, mechanism: Mechanism, serialized_key: Message,
                               format: KeySerialization, attributes: StorageAttributes)
        -> ClientResult<'c, reply::DeserializeKey, Self>;


    fn delete<'c>(
        &'c mut self,
        // mechanism: Mechanism,
        key: ObjectHandle,
    )
        -> ClientResult<'c, reply::Delete, Self>;


    fn debug_dump_store<'c>(
        &'c mut self,
    )
        -> ClientResult<'c, reply::DebugDumpStore, Self>;


    fn exists<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
    )
        -> ClientResult<'c, reply::Exists, Self>;


    fn generate_key<'c>(&'c mut self, mechanism: Mechanism, attributes: StorageAttributes)
        -> ClientResult<'c, reply::GenerateKey, Self>;


    fn read_dir_first<'c>(
        &'c mut self,
        location: StorageLocation,
        dir: PathBuf,
        not_before_filename: Option<PathBuf>,
    )
        -> ClientResult<'c, reply::ReadDirFirst, Self>;


    fn read_dir_next<'c>(
        &'c mut self,
    )
        -> ClientResult<'c, reply::ReadDirNext, Self>;


    fn read_dir_files_first<'c>(
        &'c mut self,
        location: StorageLocation,
        dir: PathBuf,
        user_attribute: Option<UserAttribute>,
    )
        -> ClientResult<'c, reply::ReadDirFilesFirst, Self>;


    fn read_dir_files_next<'c>(
        &'c mut self,
    )
        -> ClientResult<'c, reply::ReadDirFilesNext, Self>;


    fn remove_dir<'c>(&'c mut self, location: StorageLocation, path: PathBuf)
        -> ClientResult<'c, reply::RemoveFile, Self>;


    fn remove_file<'c>(&'c mut self, location: StorageLocation, path: PathBuf)
        -> ClientResult<'c, reply::RemoveFile, Self>;


    fn read_file<'c>(&'c mut self, location: StorageLocation, path: PathBuf)
        -> ClientResult<'c, reply::ReadFile, Self>;


    fn locate_file<'c>(&'c mut self, location: StorageLocation,
                           dir: Option<PathBuf>,
                           filename: PathBuf,
                           )
        -> ClientResult<'c, reply::LocateFile, Self>;


    fn write_file<'c>(
        &'c mut self,
        location: StorageLocation,
        path: PathBuf,
        data: Message,
        user_attribute: Option<UserAttribute>,
        )
        -> ClientResult<'c, reply::WriteFile, Self>;

          // - mechanism: Mechanism
          // - key: ObjectHandle
          // - format: KeySerialization

    fn serialize_key<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle, format: KeySerialization)
        -> ClientResult<'c, reply::SerializeKey, Self>;


    fn sign<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
        data: &[u8],
        format: SignatureSerialization,
    )
        -> ClientResult<'c, reply::Sign, Self>;


    fn verify<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
        message: &[u8],
        signature: &[u8],
        format: SignatureSerialization,
    )
        -> ClientResult<'c, reply::Verify, Self>;



    fn random_bytes<'c>(&'c mut self, count: usize)
        -> ClientResult<'c, reply::RandomByteBuf, Self>;


    fn hash<'c>(&'c mut self, mechanism: Mechanism, message: Message)
        -> ClientResult<'c, reply::Hash, Self>;


    fn hash_sha256<'c>(&'c mut self, message: &[u8])
        -> ClientResult<'c, reply::Hash, Self>
    {
        self.hash(Mechanism::Sha256, Message::from_slice(message).map_err(|_| ClientError::DataTooLarge)?)
    }

    fn decrypt_chacha8poly1305<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], associated_data: &[u8],
                                       nonce: &[u8], tag: &[u8])
        -> ClientResult<'c, reply::Decrypt, Self>
    {
        self.decrypt(Mechanism::Chacha8Poly1305, key.clone(), message, associated_data, nonce, tag)
    }

    fn decrypt_aes256cbc<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> ClientResult<'c, reply::Decrypt, Self>
    {
        self.decrypt(
            Mechanism::Aes256Cbc, key.clone(), message, &[], &[], &[],
        )
    }

    fn encrypt_chacha8poly1305<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], associated_data: &[u8],
                                       nonce: Option<&[u8; 12]>)
        -> ClientResult<'c, reply::Encrypt, Self>
    {
        self.encrypt(Mechanism::Chacha8Poly1305, key.clone(), message, associated_data,
            nonce.and_then(|nonce| ShortData::from_slice(nonce).ok()))
    }

    fn decrypt_tdes<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> ClientResult<'c, reply::Decrypt, Self>
    {
        self.decrypt(Mechanism::Tdes, key.clone(), message, &[], &[], &[])
    }

    fn encrypt_tdes<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> ClientResult<'c, reply::Encrypt, Self>
    {
        self.encrypt(Mechanism::Tdes, key.clone(), message, &[], None)
    }


    fn unsafe_inject_totp_key<'c>(&'c mut self, raw_key: &[u8; 20], persistence: StorageLocation)
        -> ClientResult<'c, reply::UnsafeInjectKey, Self>;


    fn unsafe_inject_tdes_key<'c>(&'c mut self, raw_key: &[u8; 24], persistence: StorageLocation)
        -> ClientResult<'c, reply::UnsafeInjectKey, Self>;

    fn generate_chacha8poly1305_key<'c>(&'c mut self, persistence: StorageLocation)
        -> ClientResult<'c, reply::GenerateKey, Self>
    {
        self.generate_key(Mechanism::Chacha8Poly1305, StorageAttributes::new().set_persistence(persistence))
    }

    fn generate_ed25519_private_key<'c>(&'c mut self, persistence: StorageLocation)
        -> ClientResult<'c, reply::GenerateKey, Self>
    {
        self.generate_key(Mechanism::Ed25519, StorageAttributes::new().set_persistence(persistence))
    }

    fn generate_hmacsha256_key<'c>(&'c mut self, persistence: StorageLocation)
        -> ClientResult<'c, reply::GenerateKey, Self>
    {
        self.generate_key(Mechanism::HmacSha256, StorageAttributes::new().set_persistence(persistence))
    }

    fn derive_ed25519_public_key<'c>(&'c mut self, private_key: &ObjectHandle, persistence: StorageLocation)
        -> ClientResult<'c, reply::DeriveKey, Self>
    {
        self.derive_key(Mechanism::Ed25519, private_key.clone(), StorageAttributes::new().set_persistence(persistence))
    }

    fn generate_p256_private_key<'c>(&'c mut self, persistence: StorageLocation)
        -> ClientResult<'c, reply::GenerateKey, Self>
    {
        self.generate_key(Mechanism::P256, StorageAttributes::new().set_persistence(persistence))
    }

    fn derive_p256_public_key<'c>(&'c mut self, private_key: &ObjectHandle, persistence: StorageLocation)
        -> ClientResult<'c, reply::DeriveKey, Self>
    {
        self.derive_key(Mechanism::P256, private_key.clone(), StorageAttributes::new().set_persistence(persistence))
    }

    fn sign_ed25519<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> ClientResult<'c, reply::Sign, Self>
    {
        self.sign(Mechanism::Ed25519, key.clone(), message, SignatureSerialization::Raw)
    }

    fn sign_hmacsha256<'c>(&'c mut self, key: &ObjectHandle, message: &[u8])
        -> ClientResult<'c, reply::Sign, Self>
    {
        self.sign(Mechanism::HmacSha256, key.clone(), message, SignatureSerialization::Raw)
    }

    // generally, don't offer multiple versions of a mechanism, if possible.
    // try using the simplest when given the choice.
    // hashing is something users can do themselves hopefully :)
    //
    // on the other hand: if users need sha256, then if the service runs in secure trustzone
    // domain, we'll maybe need two copies of the sha2 code
    fn sign_p256<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], format: SignatureSerialization)
        -> ClientResult<'c, reply::Sign, Self>
    {
        self.sign(Mechanism::P256, key.clone(), message, format)
    }

    fn sign_totp<'c>(&'c mut self, key: &ObjectHandle, timestamp: u64)
        -> ClientResult<'c, reply::Sign, Self>
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
    fn unwrap_key<'c>(&'c mut self, mechanism: Mechanism, wrapping_key: ObjectHandle, wrapped_key: Message,
                       associated_data: &[u8], attributes: StorageAttributes)
        -> ClientResult<'c, reply::UnwrapKey, Self>;

    fn unwrap_key_chacha8poly1305<'c>(&'c mut self, wrapping_key: &ObjectHandle, wrapped_key: &Message,
                       associated_data: &[u8], location: StorageLocation)
        -> ClientResult<'c, reply::UnwrapKey, Self>
    {
        self.unwrap_key(Mechanism::Chacha8Poly1305, wrapping_key.clone(), wrapped_key.clone(), associated_data,
                         StorageAttributes::new().set_persistence(location))
    }

    fn verify_ed25519<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], signature: &[u8])
        -> ClientResult<'c, reply::Verify, Self>
    {
        self.verify(Mechanism::Ed25519, key.clone(), message, signature, SignatureSerialization::Raw)
    }

    fn verify_p256<'c>(&'c mut self, key: &ObjectHandle, message: &[u8], signature: &[u8])
        -> ClientResult<'c, reply::Verify, Self>
    {
        self.verify(Mechanism::P256, key.clone(), message, signature, SignatureSerialization::Raw)
    }


          // - mechanism: Mechanism
          // - wrapping_key: ObjectHandle
          // - key: ObjectHandle
          // - associated_data: Message
    fn wrap_key<'c>(&'c mut self, mechanism: Mechanism, wrapping_key: ObjectHandle, key: ObjectHandle,
                       associated_data: &[u8])
        -> ClientResult<'c, reply::WrapKey, Self>;

    fn wrap_key_chacha8poly1305<'c>(&'c mut self, wrapping_key: &ObjectHandle, key: &ObjectHandle,
                       associated_data: &[u8])
        -> ClientResult<'c, reply::WrapKey, Self>
    {
        self.wrap_key(Mechanism::Chacha8Poly1305, wrapping_key.clone(), key.clone(), associated_data)
    }

    fn wrap_key_aes256cbc<'c>(&'c mut self, wrapping_key: &ObjectHandle, key: &ObjectHandle)
        -> ClientResult<'c, reply::WrapKey, Self>
    {
        self.wrap_key(Mechanism::Aes256Cbc, wrapping_key.clone(), key.clone(), &[])
    }

    fn confirm_user_present<'c>(&'c mut self, timeout_milliseconds: u32)
        -> ClientResult<'c, reply::RequestUserConsent, Self>;


    fn reboot<'c>(&'c mut self, to: reboot::To)
        -> ClientResult<'c, reply::Reboot, Self>;

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

pub struct FutureResult<'c, T, C: ?Sized>
where C: Client
{
    client: &'c mut C,
    __: PhantomData<T>,
}

impl<'c,T, C> FutureResult<'c, T, C>
where
    T: From<crate::api::Reply>,
    C: Client,
{
    pub fn new(client: &'c mut C) -> Self {
        Self { client: client, __: PhantomData}
    }
    pub fn poll(&mut self)
        -> core::task::Poll<core::result::Result<T, Error>>
    {
        use core::task::Poll::{Pending, Ready};
        match self.client.poll() {
            Ready(Ok(reply)) => Ready(Ok(T::from(reply))),
            Ready(Err(error)) => Ready(Err(error)),
            Pending => Pending
        }
    }

}

pub struct ClientImplementation<S: Syscall> {
    // raw: RawClient<Client<S>>,
    syscall: S,

    // RawClient:
    pub(crate) interchange: Requester<TrussedInterchange>,
    // pending: Option<Discriminant<Request>>,
    pending: Option<u8>,
}

// impl<S> From<(RawClient, S)> for Client<S>
// where S: Syscall
// {
//     fn from(input: (RawClient, S)) -> Self {
//         Self { raw: input.0, syscall: input.1 }
//     }
// }


impl<S> ClientImplementation<S>
where S: Syscall
{
    pub fn new(interchange: Requester<TrussedInterchange>, syscall: S) -> Self {
        Self { interchange: interchange, pending: None, syscall }
    }

    // call with any of `crate::api::request::*`
    pub fn request<'c, T: From<crate::api::Reply>>(&'c mut self, req: impl Into<Request>)
        // -> core::result::Result<FutureResult<'c, T, Client<S>>, ClientError>
        -> ClientResult<'c, T, Self>
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
        Ok(FutureResult::new(self))
    }


}

impl<S> Client for ClientImplementation<S>
where S: Syscall {

    fn poll(&mut self)
        -> core::task::Poll<core::result::Result<Reply, Error>>
    {
        match self.interchange.take_response() {
            Some(reply) => {
                // #[cfg(all(test, feature = "verbose-tests"))]
                // println!("got a reply: {:?}", &reply);
                match reply {
                    Ok(reply) => {
                        if Some(u8::from(&reply)) == self.pending {
                            self.pending = None;
                            core::task::Poll::Ready(Ok(reply))
                        } else  {
                            // #[cfg(all(test, feature = "verbose-tests"))]
                            info!("got: {:?}, expected: {:?}", Some(u8::from(&reply)), self.pending);
                            core::task::Poll::Ready(Err(Error::InternalError))
                        }
                    }
                    Err(error) => {
                        self.pending = None;
                        core::task::Poll::Ready(Err(error))
                    }
                }

            },
            None => core::task::Poll::Pending
        }
    }

    fn agree<'c>(
        &'c mut self, mechanism: Mechanism,
        private_key: ObjectHandle, public_key: ObjectHandle,
        attributes: StorageAttributes,
        )
        -> ClientResult<'c, reply::Agree, Self>
    {
        let r = self.request(request::Agree {
            mechanism,
            private_key,
            public_key,
            attributes,
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn derive_key<'c>(&'c mut self, mechanism: Mechanism, base_key: ObjectHandle, attributes: StorageAttributes)
        -> ClientResult<'c, reply::DeriveKey, Self>
    {
        let r = self.request(request::DeriveKey {
            mechanism,
            base_key,
            attributes,
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }

          // - mechanism: Mechanism
          // - key: ObjectHandle
          // - message: Message
          // - associated_data: ShortData
    fn encrypt<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle,
                       message: &[u8], associated_data: &[u8], nonce: Option<ShortData>)
        -> ClientResult<'c, reply::Encrypt, Self>
    {
        let message = Message::from_slice(message).map_err(|_| ClientError::DataTooLarge)?;
        let associated_data = ShortData::from_slice(associated_data).map_err(|_| ClientError::DataTooLarge)?;
        let r = self.request(request::Encrypt { mechanism, key, message, associated_data, nonce })?;
        r.client.syscall.syscall();
        Ok(r)
    }

          // - mechanism: Mechanism
          // - key: ObjectHandle
          // - message: Message
          // - associated_data: ShortData
          // - nonce: ShortData
          // - tag: ShortData
    fn decrypt<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle,
                       message: &[u8], associated_data: &[u8],
                       nonce: &[u8], tag: &[u8],
                       )
        -> ClientResult<'c, reply::Decrypt, Self>
    {
        let message = Message::from_slice(message).map_err(|_| ClientError::DataTooLarge)?;
        let associated_data = Message::from_slice(associated_data).map_err(|_| ClientError::DataTooLarge)?;
        let nonce = ShortData::from_slice(nonce).map_err(|_| ClientError::DataTooLarge)?;
        let tag = ShortData::from_slice(tag).map_err(|_| ClientError::DataTooLarge)?;
        let r = self.request(request::Decrypt { mechanism, key, message, associated_data, nonce, tag })?;
        r.client.syscall.syscall();
        Ok(r)
    }

          // - mechanism: Mechanism
          // - serialized_key: Message
          // - format: KeySerialization
          // - attributes: StorageAttributes
    fn deserialize_key<'c>(&'c mut self, mechanism: Mechanism, serialized_key: Message,
                               format: KeySerialization, attributes: StorageAttributes)
        -> ClientResult<'c, reply::DeserializeKey, Self>
    {
        let r = self.request(request::DeserializeKey {
            mechanism, serialized_key, format, attributes
        } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn delete<'c>(
        &'c mut self,
        // mechanism: Mechanism,
        key: ObjectHandle,
    )
        -> ClientResult<'c, reply::Delete, Self>
    {
        let r = self.request(request::Delete {
            key,
            // mechanism,
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn debug_dump_store<'c>(
        &'c mut self,
    )
        -> ClientResult<'c, reply::DebugDumpStore, Self>
    {
        let r = self.request(request::DebugDumpStore {})?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn exists<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
    )
        -> ClientResult<'c, reply::Exists, Self>
    {
        let r = self.request(request::Exists {
            key,
            mechanism,
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn generate_key<'c>(&'c mut self, mechanism: Mechanism, attributes: StorageAttributes)
        -> ClientResult<'c, reply::GenerateKey, Self>
    {
        let r = self.request(request::GenerateKey {
            mechanism,
            attributes,
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn read_dir_first<'c>(
        &'c mut self,
        location: StorageLocation,
        dir: PathBuf,
        not_before_filename: Option<PathBuf>,
    )
        -> ClientResult<'c, reply::ReadDirFirst, Self>
    {
        let r = self.request(request::ReadDirFirst { location, dir, not_before_filename } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn read_dir_next<'c>(
        &'c mut self,
    )
        -> ClientResult<'c, reply::ReadDirNext, Self>
    {
        let r = self.request(request::ReadDirNext {} )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn read_dir_files_first<'c>(
        &'c mut self,
        location: StorageLocation,
        dir: PathBuf,
        user_attribute: Option<UserAttribute>,
    )
        -> ClientResult<'c, reply::ReadDirFilesFirst, Self>
    {
        let r = self.request(request::ReadDirFilesFirst { dir, location, user_attribute } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn read_dir_files_next<'c>(
        &'c mut self,
    )
        -> ClientResult<'c, reply::ReadDirFilesNext, Self>
    {
        let r = self.request(request::ReadDirFilesNext {} )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn remove_dir<'c>(&'c mut self, location: StorageLocation, path: PathBuf)
        -> ClientResult<'c, reply::RemoveFile, Self>
    {
        let r = self.request(request::RemoveDir { location, path } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn remove_file<'c>(&'c mut self, location: StorageLocation, path: PathBuf)
        -> ClientResult<'c, reply::RemoveFile, Self>
    {
        let r = self.request(request::RemoveFile { location, path } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn read_file<'c>(&'c mut self, location: StorageLocation, path: PathBuf)
        -> ClientResult<'c, reply::ReadFile, Self>
    {
        let r = self.request(request::ReadFile { location, path } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn locate_file<'c>(&'c mut self, location: StorageLocation,
                           dir: Option<PathBuf>,
                           filename: PathBuf,
                           )
        -> ClientResult<'c, reply::LocateFile, Self>
    {
        let r = self.request(request::LocateFile { location, dir, filename } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn write_file<'c>(
        &'c mut self,
        location: StorageLocation,
        path: PathBuf,
        data: Message,
        user_attribute: Option<UserAttribute>,
        )
        -> ClientResult<'c, reply::WriteFile, Self>
    {
        let r = self.request(request::WriteFile {
            location, path, data,
            user_attribute,
        } )?;
        r.client.syscall.syscall();
        Ok(r)
    }
          // - mechanism: Mechanism
          // - key: ObjectHandle
          // - format: KeySerialization

    fn serialize_key<'c>(&'c mut self, mechanism: Mechanism, key: ObjectHandle, format: KeySerialization)
        -> ClientResult<'c, reply::SerializeKey, Self>
    {
        let r = self.request(request::SerializeKey {
            key,
            mechanism,
            format,
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn sign<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
        data: &[u8],
        format: SignatureSerialization,
    )
        -> ClientResult<'c, reply::Sign, Self>
    {
        let r = self.request(request::Sign {
            key,
            mechanism,
            message: ByteBuf::from_slice(data).map_err(|_| ClientError::DataTooLarge)?,
            format,
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn verify<'c>(
        &'c mut self,
        mechanism: Mechanism,
        key: ObjectHandle,
        message: &[u8],
        signature: &[u8],
        format: SignatureSerialization,
    )
        -> ClientResult<'c, reply::Verify, Self>
    {
        let r = self.request(request::Verify {
            mechanism,
            key,
            message: Message::from_slice(&message).expect("all good"),
            signature: Signature::from_slice(&signature).expect("all good"),
            format,
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }


    fn random_bytes<'c>(&'c mut self, count: usize)
        -> ClientResult<'c, reply::RandomByteBuf, Self>
    {
        let r = self.request(request::RandomByteBuf { count } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn hash<'c>(&'c mut self, mechanism: Mechanism, message: Message)
        -> ClientResult<'c, reply::Hash, Self>
    {
        let r = self.request(request::Hash { mechanism, message } )?;
        r.client.syscall.syscall();
        Ok(r)
    }


    fn unsafe_inject_totp_key<'c>(&'c mut self, raw_key: &[u8; 20], persistence: StorageLocation)
        -> ClientResult<'c, reply::UnsafeInjectKey, Self>
    {
        info_now!("{}B: raw key: {:X?}", raw_key.len(), raw_key);
        let r = self.request(request::UnsafeInjectKey {
            mechanism: Mechanism::Totp,
            raw_key: ShortData::from_slice(raw_key).unwrap(),
            attributes: StorageAttributes::new().set_persistence(persistence),
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn unsafe_inject_tdes_key<'c>(&'c mut self, raw_key: &[u8; 24], persistence: StorageLocation)
        -> ClientResult<'c, reply::UnsafeInjectKey, Self>
    {
        let r = self.request(request::UnsafeInjectKey {
            mechanism: Mechanism::Tdes,
            raw_key: ShortData::from_slice(raw_key).unwrap(),
            attributes: StorageAttributes::new().set_persistence(persistence),
        })?;
        r.client.syscall.syscall();
        Ok(r)
    }

          // - mechanism: Mechanism
          // - wrapping_key: ObjectHandle
          // - wrapped_key: Message
          // - associated_data: Message
    fn unwrap_key<'c>(&'c mut self, mechanism: Mechanism, wrapping_key: ObjectHandle, wrapped_key: Message,
                       associated_data: &[u8], attributes: StorageAttributes)
        -> ClientResult<'c, reply::UnwrapKey, Self>
    {
        let associated_data = Message::from_slice(associated_data).map_err(|_| ClientError::DataTooLarge)?;
        let r = self.request(request::UnwrapKey { mechanism, wrapping_key, wrapped_key, associated_data, attributes })?;
        r.client.syscall.syscall();
        Ok(r)
    }

          // - mechanism: Mechanism
          // - wrapping_key: ObjectHandle
          // - key: ObjectHandle
          // - associated_data: Message
    fn wrap_key<'c>(&'c mut self, mechanism: Mechanism, wrapping_key: ObjectHandle, key: ObjectHandle,
                       associated_data: &[u8])
        -> ClientResult<'c, reply::WrapKey, Self>
    {
        let associated_data = Message::from_slice(associated_data).map_err(|_| ClientError::DataTooLarge)?;
        let r = self.request(request::WrapKey { mechanism, wrapping_key, key, associated_data })?;
        r.client.syscall.syscall();
        Ok(r)
    }


    fn confirm_user_present<'c>(&'c mut self, timeout_milliseconds: u32)
        -> ClientResult<'c, reply::RequestUserConsent, Self>
    {
        let r = self.request(request::RequestUserConsent {
            level: consent::Level::Normal,
            timeout_milliseconds,
        } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

    fn reboot<'c>(&'c mut self, to: reboot::To)
        -> ClientResult<'c, reply::Reboot, Self>
    {
        let r = self.request(request::Reboot {
            to: to,
        } )?;
        r.client.syscall.syscall();
        Ok(r)
    }

}
