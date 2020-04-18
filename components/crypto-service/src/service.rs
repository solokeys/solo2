#[cfg(feature = "semihosting")]
use cortex_m_semihosting::hprintln;
pub use embedded_hal::blocking::rng::Read as RngRead;
use heapless_bytes::Bytes;
use littlefs2::path::Path;


use crate::api::*;
use crate::config::*;
use crate::error::Error;
use crate::mechanisms;
use crate::storage::{self, *};
use crate::types::*;

pub use crate::pipe::ServiceEndpoint;

// #[macro_use]
// mod macros;

macro_rules! rpc_trait { ($($Name:ident, $name:ident,)*) => { $(

    pub trait $Name<R: RngRead, S: Store> {
        fn $name(_resources: &mut ServiceResources<R, S>, _request: request::$Name)
        -> Result<reply::$Name, Error> { Err(Error::MechanismNotAvailable) }
    }
)* } }

rpc_trait! {
    Agree, agree,
    Decrypt, decrypt,
    DeriveKey, derive_key,
    DeserializeKey, deserialize_key,
    Encrypt, encrypt,
    Exists, exists,
    GenerateKey, generate_key,
    Hash, hash,
    SerializeKey, serialize_key,
    Sign, sign,
    UnwrapKey, unwrap_key,
    Verify, verify,
    // TODO: can the default implementation be implemented in terms of Encrypt?
    WrapKey, wrap_key,
}

// pub fn cbor_serialize<T: serde::Serialize>(
//     object: &T,
//     buffer: &mut [u8],
// ) -> core::result::Result<usize, serde_cbor::Error> {
//     let writer = serde_cbor::ser::SliceWrite::new(buffer);
//     let mut ser = serde_cbor::Serializer::new(writer);

//     object.serialize(&mut ser)?;

//     let writer = ser.into_inner();
//     let size = writer.bytes_written();

//     Ok(size)
// }

// pub fn cbor_deserialize<'de, T: serde::Deserialize<'de>>(
//     buffer: &'de [u8],
// ) -> core::result::Result<T, ctapcbor::error::Error> {
//     ctapcbor::de::from_bytes(buffer)
// }

// associated keys end up namespaced under "/fido2"
// example: "/fido2/keys/2347234"
// let (mut fido_endpoint, mut fido2_client) = Client::new("fido2");
// let (mut piv_endpoint, mut piv_client) = Client::new("piv");

pub struct ServiceResources<R, S>
where
    R: RngRead,
	S: Store,
{
    pub(crate) rng: R,
    pub(crate) store: S,
    currently_serving: ClientId,
}

impl<R: RngRead, S: Store> ServiceResources<R, S> {

    pub fn new(
        rng: R,
        store: S,
    ) -> Self {

        Self { rng, store, currently_serving: heapless::Vec::new() }
    }
}

// pub(crate) fn load_serialized_key<'s, S: LfsStorage>(fs: &mut Filesystem<'s, S>, path: &[u8], buf: &mut [u8]) -> Result<usize, Error> {

//     use littlefs2::fs::File;
//     use littlefs2::io::Read;

//     let
//     fs.open_file_and_then(path, |file| {
//     let mut alloc = File::allocate();
//     let mut file = File::open(&path[..], &mut alloc, fs)
//         .map_err(|_| Error::FilesystemReadFailure)?;

//     // hprintln!("reading it").ok();
//     let size = file.read(buf)
//         .map_err(|_| Error::FilesystemReadFailure)?;

//     Ok(size)
// }

// pub fn find_next(
//     fs: &mut Filesystem<'s, S>,
//     dir: &[u8],
//     user_attribute: Option<UserAttribute>,
//     previous: Option<ObjectHandle>,
// )
//     -> Result<Option<ObjectHandle>, Error>
// {
//     let mut read_dir = fs.read_dir(dir, &mut storage).unwrap();
// }

pub struct Service<'a, R, S>
where
    R: RngRead,
    S: Store,
{
    eps: Vec<ServiceEndpoint<'a>, MAX_SERVICE_CLIENTS>,
    resources: ServiceResources<R, S>,
}

// need to be able to send crypto service to an interrupt handler
unsafe impl<R: RngRead, S: Store> Send for Service<'_, R, S> {}

impl<R: RngRead, S: Store> ServiceResources<R, S> {

    pub fn load_key_unchecked(&mut self, path: &[u8]) -> Result<(SerializedKey, StorageLocation), Error> {
        storage::load_key_unchecked(self.store, path)
    }

    pub fn load_key(&mut self, path: &[u8], kind: KeyKind, key_bytes: &mut [u8])
        -> Result<StorageLocation, Error>
    {
        storage::load_key(self.store, path, kind, key_bytes)
    }

    pub fn store_key(&mut self, to: StorageLocation, path: &[u8], kind: KeyKind, key_bytes: &[u8])
        -> Result<(), Error>
    {
        storage::store_key(self.store, to, path, kind, key_bytes)
    }

    pub fn reply_to(&mut self, request: Request) -> Result<Reply, Error> {
        // TODO: what we want to do here is map an enum to a generic type
        // Is there a nicer way to do this?
        // debug!("crypto-service request: {:?}", &request).ok();
        // debug!("IFS/EFS/VFS available BEFORE: {}/{}/{}",
        //       self.tri.ifs.available_blocks().unwrap(),
        //       self.tri.efs.available_blocks().unwrap(),
        //       self.tri.vfs.available_blocks().unwrap(),
        // ).ok();
        #[cfg(feature = "deep-semihosting-logs")]
        hprintln!("crypto-service request: {:?}", &request).ok();
        #[cfg(feature = "deep-semihosting-logs")]
        hprintln!("IFS/EFS/VFS available BEFORE: {}/{}/{}",
              self.store.ifs().available_blocks().unwrap(),
              self.store.efs().available_blocks().unwrap(),
              self.store.vfs().available_blocks().unwrap(),
        ).ok();
        match request {
            Request::DummyRequest => {
                // #[cfg(test)]
                // println!("got a dummy request!");
                Ok(Reply::DummyReply)
            },

            Request::Agree(request) => {
                match request.mechanism {

                    Mechanism::P256 => mechanisms::P256::agree(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Agree(reply))
            },

            Request::Decrypt(request) => {
                match request.mechanism {

                    Mechanism::Aes256Cbc => mechanisms::Aes256Cbc::decrypt(self, request),
                    Mechanism::Chacha8Poly1305 => mechanisms::Chacha8Poly1305::decrypt(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Decrypt(reply))
            },

            Request::DeriveKey(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::derive_key(self, request),
                    Mechanism::P256 => mechanisms::P256::derive_key(self, request),
                    Mechanism::Sha256 => mechanisms::Sha256::derive_key(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::DeriveKey(reply))
            },

            Request::DeserializeKey(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::deserialize_key(self, request),
                    Mechanism::P256 => mechanisms::P256::deserialize_key(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::DeserializeKey(reply))
            }

            Request::Encrypt(request) => {
                match request.mechanism {

                    Mechanism::Aes256Cbc => mechanisms::Aes256Cbc::encrypt(self, request),
                    Mechanism::Chacha8Poly1305 => mechanisms::Chacha8Poly1305::encrypt(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Encrypt(reply))
            },

            Request::Delete(request) => {
                let success = {
                    let path = self.prepare_path_for_key(KeyType::Private, &request.key.object_id)?;
                    match storage::delete_key(self.store, &path) {
                        true => true,
                        false => {
                            let path = self.prepare_path_for_key(KeyType::Public, &request.key.object_id)?;
                            match storage::delete_key(self.store, &path) {
                                true => true,
                                false => {
                                    let path = self.prepare_path_for_key(KeyType::Secret, &request.key.object_id)?;
                                    storage::delete_key(self.store, &path)
                                }
                            }
                        }
                    }
                };
                Ok(Reply::Delete(reply::Delete { success } ))
            },

            Request::Exists(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::exists(self, request),
                    Mechanism::P256 => mechanisms::P256::exists(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Exists(reply))
            },

            Request::GenerateKey(request) => {
                match request.mechanism {
                    Mechanism::Chacha8Poly1305 => mechanisms::Chacha8Poly1305::generate_key(self, request),
                    Mechanism::Ed25519 => mechanisms::Ed25519::generate_key(self, request),
                    Mechanism::HmacSha256 => mechanisms::HmacSha256::generate_key(self, request),
                    Mechanism::P256 => mechanisms::P256::generate_key(self, request),
                    _ => Err(Error::MechanismNotAvailable),
                }.map(|reply| Reply::GenerateKey(reply))
            },

            Request::Hash(request) => {
                match request.mechanism {

                    Mechanism::Sha256 => mechanisms::Sha256::hash(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Hash(reply))
            },

            Request::ListBlobsFirst(request) => {
                // TODO: ergonooomics

                let mut path: Path<S::I> = Path::new(b"/");//Bytes::<MAX_PATH_LENGTH>::new();
                hprintln!("current: {:?}", &self.currently_serving).ok();
                path.push(&self.currently_serving[..]);

                hprintln!("prefix: {:?}", &request.prefix);
                if let Some(prefix) = request.prefix.clone() {
                    path.push(&prefix.0[..]);
                }

                #[cfg(feature = "semihosting")]
                hprintln!("listing blobs in {:?}", &path).ok();

                let fs = self.store.ifs();

                let entry = fs.read_dir_and_then(&path[..], |dir| {
                    for entry in dir {
                        let entry = entry.unwrap();
                        // let entry = entry?;//.map_err(|_| Error::InternalError)?;
                        if entry.file_type().is_dir() {
                            #[cfg(feature = "semihosting")]
                            hprintln!("a skipping subdirectory {:?}", &entry.file_name()).ok();
                            continue;
                        }

                        // hprintln!("done skipping").ok();

                        let name = entry.file_name();
                        #[cfg(feature = "semihosting")]
                        hprintln!("first file found: {:?}", core::str::from_utf8(&name[..]).unwrap()).ok();

                        if let Some(user_attribute) = request.user_attribute.as_ref() {
                            let mut path = path.clone();
                            path.push(&name[..]);
                            let attribute = fs.attribute(&path[..], crate::config::USER_ATTRIBUTE_NUMBER)
                                .map_err(|e| {
                                    info!("error getting attribute: {:?}", &e).ok();
                                    littlefs2::io::Error::Io
                                }
                            )?;

                            match attribute {
                                None => continue,
                                Some(attribute) => {
                                    if user_attribute != attribute.data() {
                                        continue;
                                    }
                                }
                            }
                            return Ok(entry)
                        }
                    }

                    Err(littlefs2::io::Error::NoSuchEntry)

                }).map_err(|_| Error::InternalError)?;

                let unique_id = UniqueId::try_from_hex(&entry.file_name()).unwrap();
                hprintln!("unique id: {:?}", &unique_id).ok();

                Ok(Reply::ListBlobsFirst(reply::ListBlobsFirst {
                    // maybe return size too?
                    id: ObjectHandle { object_id: unique_id },
                    data: Message::new(),
                } ))
            }

            Request::LoadBlob(request) => {
                let path = self.blob_path(&request.prefix, Some(&request.id.object_id))?;
                let mut data = Message::new();
                data.resize_to_capacity();
                let data: Message = match request.location {
                    StorageLocation::Internal => self.store.ifs().read(&path[..]),
                    StorageLocation::External => self.store.efs().read(&path[..]),
                    StorageLocation::Volatile => self.store.vfs().read(&path[..]),
                }.map_err(|_| Error::InternalError)?.into();
                // data.resize_default(size).map_err(|_| Error::InternalError)?;
                Ok(Reply::LoadBlob(reply::LoadBlob { data } ))
            }

            Request::RandomBytes(request) => {
                if request.count < 1024 {
                    let mut bytes = Message::new();
                    bytes.resize_default(request.count).unwrap();
                    self.rng.read(&mut bytes)
                        .map_err(|_| Error::EntropyMalfunction)?;
                    Ok(Reply::RandomBytes(reply::RandomBytes { bytes } ))
                } else {
                    Err(Error::MechanismNotAvailable)
                }
            }

            Request::SerializeKey(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::serialize_key(self, request),
                    Mechanism::P256 => mechanisms::P256::serialize_key(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::SerializeKey(reply))
            }

            Request::Sign(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::sign(self, request),
                    Mechanism::HmacSha256 => mechanisms::HmacSha256::sign(self, request),
                    Mechanism::P256 => mechanisms::P256::sign(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Sign(reply))
            },

            Request::StoreBlob(request) => {
                let blob_id = self.generate_unique_id()?;
                let path = self.blob_path(&request.prefix, Some(&blob_id))?;
                // hprintln!("saving blob to {:?}", &path).ok();
                info!("StoreBlob of size {}", request.data.len()).ok();
                match request.attributes.persistence {
                    StorageLocation::Internal => store_serialized_key(
                        self.store.ifs(), &path, &request.data, request.user_attribute),
                    StorageLocation::External => store_serialized_key(
                        self.store.efs(), &path, &request.data, request.user_attribute),
                    StorageLocation::Volatile => store_serialized_key(
                        self.store.vfs(), &path, &request.data, request.user_attribute),
                }?;
                Ok(Reply::StoreBlob(reply::StoreBlob { blob: ObjectHandle { object_id: blob_id } }))
            }

            Request::UnwrapKey(request) => {
                match request.mechanism {

                    Mechanism::Chacha8Poly1305 => mechanisms::Chacha8Poly1305::unwrap_key(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::UnwrapKey(reply))
            }

            Request::Verify(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::verify(self, request),
                    Mechanism::P256 => mechanisms::P256::verify(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Verify(reply))
            },

            Request::WrapKey(request) => {
                match request.mechanism {

                    Mechanism::Aes256Cbc => mechanisms::Aes256Cbc::wrap_key(self, request),
                    Mechanism::Chacha8Poly1305 => mechanisms::Chacha8Poly1305::wrap_key(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::WrapKey(reply))
            },


            _ => {
                // #[cfg(test)]
                // println!("todo: {:?} request!", &request);
                Err(Error::RequestNotAvailable)
            },
        }
    }

    pub fn prepare_path_for_key(&mut self, key_type: KeyType, id: &UniqueId)
        -> Result<Bytes<MAX_PATH_LENGTH>, Error> {
        let mut path = Bytes::<MAX_PATH_LENGTH>::new();
        path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
        path.extend_from_slice(&self.currently_serving).map_err(|_| Error::InternalError)?;
        // #[cfg(all(test, feature = "verbose-tests"))]
        // #[cfg(test)]
        // println!("creating dir {:?}", &path);
        // self.pfs.create_dir(path.as_ref()).map_err(|_| Error::FilesystemWriteFailure)?;

        path.extend_from_slice(match key_type {
            KeyType::Private => b"/private",
            KeyType::Public => b"/public",
            KeyType::Secret => b"/secret",
        }).map_err(|_| Error::InternalError)?;

        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("creating dir {:?}", &path);
        // self.pfs.create_dir(path.as_ref()).map_err(|_| Error::FilesystemWriteFailure)?;
        path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
        path.extend_from_slice(&id.hex()).map_err(|_| Error::InternalError)?;
        Ok(path)
    }

    pub fn blob_path(&mut self, prefix: &Option<Letters>, id: Option<&UniqueId>)
        -> Result<Bytes<MAX_PATH_LENGTH>, Error> {
        let mut path = Bytes::<MAX_PATH_LENGTH>::new();

        path.extend_from_slice(&self.currently_serving).map_err(|_| Error::InternalError)?;
        path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;

        if let Some(prefix) = &prefix {
            if !prefix.0.iter().all(|b| *b >= b'a' && *b <= b'z') {
                return Err(crate::error::Error::NotJustLetters);
            }
            path.extend_from_slice(&prefix.0).map_err(|_| Error::InternalError)?;
            path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
        }

        // const HEX_CHARS: &[u8] = b"0123456789abcdef";
        // for byte in id.iter() {
        //     hprintln!("{}", &byte).ok();
        //     path.push(HEX_CHARS[(byte >> 4) as usize]).map_err(|_| Error::InternalError)?;
        //     path.push(HEX_CHARS[(byte & 0xf) as usize]).map_err(|_| Error::InternalError)?;
        // }
        if let Some(id) = id {
            path.extend_from_slice(&id.hex()).map_err(|_| Error::InternalError)?;
        }
        Ok(path)
    }

    pub fn generate_unique_id(&mut self) -> Result<UniqueId, Error> {
        let mut unique_id = [0u8; 16];

        self.rng.read(&mut unique_id)
            .map_err(|_| Error::EntropyMalfunction)?;

        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("unique id {:?}", &unique_id);
        Ok(UniqueId(Bytes::try_from_slice(&unique_id).unwrap()))
    }

}

impl<R: RngRead, S: Store> Service<'_, R, S> {

    pub fn new(
        rng: R,
        store: S,
    )
        -> Self
    {
        let resources = ServiceResources::new(rng, store);
        Self { eps: Vec::new(), resources }
    }
}

impl<'a, R: RngRead, S: Store> Service<'a, R, S> {

    pub fn add_endpoint(&mut self, ep: ServiceEndpoint<'a>) -> Result<(), ServiceEndpoint> {
        self.eps.push(ep)
    }

    // process one request per client which has any
    pub fn process(&mut self) {
        // split self since we iter-mut over eps and need &mut of the other resources
        let eps = &mut self.eps;
        let mut resources = &mut self.resources;

        for ep in eps.iter_mut() {
            if !ep.send.ready() {
                continue;
            }
            if let Some(request) = ep.recv.dequeue() {
                // #[cfg(test)] println!("service got request: {:?}", &request);

                resources.currently_serving.clear();
                resources.currently_serving.extend_from_slice(&ep.client_id);
                    // &ep.client_id;
                let reply_result = resources.reply_to(request);
                // #[cfg(test)] println!("service made reply: {:?}", &reply_result);

                ep.send.enqueue(reply_result).ok();

            }
        }
        // debug!("IFS/EFS/VFS available AFTER: {}/{}/{}",
        //       self.resources.tri.ifs.available_blocks().unwrap(),
        //       self.resources.tri.efs.available_blocks().unwrap(),
        //       self.resources.tri.vfs.available_blocks().unwrap(),
        // ).ok();
        #[cfg(feature = "deep-semihosting-logs")]
        hprintln!("IFS/EFS/VFS available AFTER: {}/{}/{}",
              self.resources.store.ifs().available_blocks().unwrap(),
              self.resources.store.efs().available_blocks().unwrap(),
              self.resources.store.vfs().available_blocks().unwrap(),
        ).ok();
    }
}

impl<R, S> crate::pipe::Syscall for &mut Service<'_, R, S>
where
    R: RngRead,
    S: Store,
{
    fn syscall(&mut self) {
        self.process();
    }
}
