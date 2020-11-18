use core::marker::PhantomData;

use interchange::Requester;

use crate::logger::{info, blocking};
use crate::api::*;
use crate::error::*;
use crate::pipe::TrussedInterchange;
use crate::types::*;

use crate::traits;
pub use crate::traits::client::{ClientError, ClientResult};
pub use crate::traits::platform::Syscall;

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
where C: traits::client::Client
{
    client: &'c mut C,
    __: PhantomData<T>,
}

impl<'c,T, C> FutureResult<'c, T, C>
where
    T: From<crate::api::Reply>,
    C: traits::client::Client,
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

pub struct Client<S: Syscall> {
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


impl<S> Client<S>
where S: Syscall
{
    pub fn new(interchange: Requester<TrussedInterchange>, syscall: S) -> Self {
        Self { interchange: interchange, pending: None, syscall }
    }

    // call with any of `crate::api::request::*`
    fn request<'c, T: From<crate::api::Reply>>(&'c mut self, req: impl Into<Request>)
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

                                //   core::result::Result<FutureResult<'c, T, Client<S>>, ClientError>

impl<S> traits::client::Client for Client<S>
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
                            info!("got: {:?}, expected: {:?}", Some(u8::from(&reply)), self.pending).ok();
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
        blocking::info!("{}B: raw key: {:X?}", raw_key.len(), raw_key).ok();
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
