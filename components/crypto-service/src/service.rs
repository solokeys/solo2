use core::convert::{TryFrom, TryInto};

#[cfg(feature = "semihosting")]
use cortex_m_semihosting::hprintln;
pub use embedded_hal::blocking::rng::Read as RngRead;
use heapless_bytes::Bytes;
use littlefs2::path::{Path, PathBuf};


use crate::api::*;
use crate::config::*;
use crate::error::Error;
use crate::mechanisms;
use crate::store::{self, *};
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

// associated keys end up namespaced under "/fido2"
// example: "/fido2/keys/2347234"
// let (mut fido_endpoint, mut fido2_client) = Client::new("fido2");
// let (mut piv_endpoint, mut piv_client) = Client::new("piv");

struct ReadDirFilesState {
    request: request::ReadDirFilesFirst,
    last: PathBuf,
}

pub struct ServiceResources<R, S>
where
    R: RngRead,
	S: Store,
{
    pub(crate) rng: R,
    pub(crate) store: S,
    // Option?
    currently_serving: ClientId,
    // TODO: how/when to clear
    read_dir_files_state: Option<ReadDirFilesState>,
}

impl<R: RngRead, S: Store> ServiceResources<R, S> {

    pub fn new(
        rng: R,
        store: S,
    ) -> Self {

        Self {
            rng,
            store,
            currently_serving: PathBuf::new(),
            read_dir_files_state: None,
        }
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
                let key_types = [
                    KeyType::Private,
                    KeyType::Public,
                    KeyType::Secret,
                ];

                let locations = [
                    StorageLocation::Internal,
                    StorageLocation::External,
                    StorageLocation::Volatile,
                ];

                let success = key_types.iter().any(|key_type| {
                    let path = self.key_path(*key_type, &request.key.object_id);
                    locations.iter().any(|location| {
                        store::delete(self.store, *location, &path)
                    })
                });

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

            Request::ReadDirFilesFirst(request) => {
                // TODO: ergonooomics

                assert!(request.location == StorageLocation::Internal);

                let mut path = self.namespace_path(&request.dir);

                #[cfg(feature = "semihosting")]
                hprintln!("listing blobs in {:?}", &path).ok();

                let fs = self.store.ifs();

                let entry = fs.read_dir_and_then(&path, |dir| {
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
                        hprintln!("first file found: {:?}", name.as_ref()).ok();

                        if let Some(user_attribute) = request.user_attribute.as_ref() {
                            let mut path = path.clone();
                            path.push(name);
                            let attribute = fs.attribute(&path, crate::config::USER_ATTRIBUTE_NUMBER)
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

                Ok(Reply::ReadDirFilesFirst(reply::ReadDirFilesFirst {
                    data: Some(Message::new()),
                } ))
            }

            Request::ReadFile(request) => {
                // let path = self.blob_path(&request.path, Some(&request.id.object_id))?;
                let path = self.namespace_path(&request.path);
                let mut data = Message::new();
                data.resize_to_capacity();
                let data: Message = match request.location {
                    StorageLocation::Internal => self.store.ifs().read(&path),
                    StorageLocation::External => self.store.efs().read(&path),
                    StorageLocation::Volatile => self.store.vfs().read(&path),
                }.map_err(|_| Error::InternalError)?.into();
                // data.resize_default(size).map_err(|_| Error::InternalError)?;
                Ok(Reply::ReadFile(reply::ReadFile { data } ))
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

            Request::WriteFile(request) => {
                let path = self.namespace_path(&request.path);
                info!("WriteFile of size {}", request.data.len()).ok();
                store::store(self.store, request.location, &path, &request.data)?;
                Ok(Reply::WriteFile(reply::WriteFile {}))
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

    // pub fn load_key_unchecked(&mut self, path: &[u8]) -> Result<(SerializedKey, StorageLocation), Error> {
    //     store::load_key_unchecked(self.store, path)
    // }

    // pub fn load_key(&mut self, path: &[u8], kind: KeyKind, key_bytes: &mut [u8])
    //     -> Result<StorageLocation, Error>
    // {
    //     store::load_key(self.store, path, kind, key_bytes)
    // }

    // pub fn store_key(&mut self, to: StorageLocation, path: &[u8], kind: KeyKind, key_bytes: &[u8])
    //     -> Result<(), Error>
    // {
    //     store::store_key(self.store, to, path, kind, key_bytes)
    // }

    // This and the following method are here, because ServiceResources knows
    // the current "client", while Store does not
    //
    // TODO: This seems like a design problem
    pub fn namespace_path(&self, path: &Path) -> PathBuf {
        // TODO: check no escapes!
        let mut namespaced_path = PathBuf::new();
        namespaced_path.push(&self.currently_serving);
        namespaced_path.push(path);
        namespaced_path
    }

    pub fn key_path(&self, key_type: KeyType, key_id: &UniqueId) -> PathBuf {
        let mut path = PathBuf::new();
        path.push(match key_type {
            KeyType::Private => b"private\0".try_into().unwrap(),
            KeyType::Public => b"public\0".try_into().unwrap(),
            KeyType::Secret => b"secret\0".try_into().unwrap(),
        });
        path.push(&PathBuf::from(&key_id.hex()));
        self.namespace_path(&path)
    }

    pub fn store_key(&mut self, location: StorageLocation, key_type: KeyType, key_kind: KeyKind, key_material: &[u8]) -> Result<UniqueId, Error> {
        let serialized_key = SerializedKey::try_from((key_kind, key_material))?;

        let mut buf = [0u8; 128];
        crate::cbor_serialize(&serialized_key, &mut buf).map_err(|_| Error::CborError)?;

        let key_id = self.generate_unique_id()?;
        let path = self.key_path(key_type, &key_id);

        store::store(self.store, location, &path, &buf)?;

        Ok(key_id)
    }

    pub fn overwrite_key(&self, location: StorageLocation, key_type: KeyType, key_kind: KeyKind, key_id: &UniqueId, key_material: &[u8]) -> Result<(), Error> {
        let serialized_key = SerializedKey::try_from((key_kind, key_material))?;

        let mut buf = [0u8; 128];
        crate::cbor_serialize(&serialized_key, &mut buf).map_err(|_| Error::CborError)?;

        let path = self.key_path(key_type, key_id);

        store::store(self.store, location, &path, &buf)?;

        Ok(())
    }

    pub fn key_id_location(&self, key_type: KeyType, key_id: &UniqueId) -> Option<StorageLocation> {
        let path = self.key_path(key_type, key_id);

        if path.exists(&self.store.vfs()) {
            return Some(StorageLocation::Volatile);
        }

        if path.exists(&self.store.ifs()) {
            return Some(StorageLocation::Internal);
        }

        if path.exists(&self.store.efs()) {
            return Some(StorageLocation::External);
        }

        None
    }

    pub fn exists_key(&self, key_type: KeyType, key_kind: Option<KeyKind>, key_id: &UniqueId)
        -> bool  {
        self.load_key(KeyType::Private, Some(KeyKind::Ed25519), key_id).is_ok()
    }

    pub fn load_key(&self, key_type: KeyType, key_kind: Option<KeyKind>, key_id: &UniqueId)
        -> Result<SerializedKey, Error>  {

        let path = self.key_path(key_type, key_id);

        let location = match self.key_id_location(key_type, key_id) {
            Some(location) => location,
            None => return Err(Error::NoSuchKey),
        };

        let bytes: Vec<u8, consts::U128> = store::read(self.store, location, &path)?;

        let serialized_key: SerializedKey = crate::cbor_deserialize(&bytes).map_err(|_| Error::CborError)?;

        if let Some(kind) = key_kind {
            if serialized_key.kind != kind {
                hprintln!("wrong key kind, expected {:?} got {:?}", &kind, &serialized_key.kind).ok();
                Err(Error::WrongKeyKind)?;
            }
        }

        Ok(serialized_key)
    }

    // pub fn prepare_path_for_key(&mut self, key_type: KeyType, id: &UniqueId)
    //     -> Result<Bytes<MAX_PATH_LENGTH>, Error> {
    //     let mut path = Bytes::<MAX_PATH_LENGTH>::new();
    //     path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
    //     path.extend_from_slice(&self.currently_serving).map_err(|_| Error::InternalError)?;
    //     // #[cfg(all(test, feature = "verbose-tests"))]
    //     // #[cfg(test)]
    //     // println!("creating dir {:?}", &path);
    //     // self.pfs.create_dir(path.as_ref()).map_err(|_| Error::FilesystemWriteFailure)?;

    //     path.extend_from_slice(match key_type {
    //         KeyType::Private => b"/private",
    //         KeyType::Public => b"/public",
    //         KeyType::Secret => b"/secret",
    //     }).map_err(|_| Error::InternalError)?;

    //     // #[cfg(all(test, feature = "verbose-tests"))]
    //     // println!("creating dir {:?}", &path);
    //     // self.pfs.create_dir(path.as_ref()).map_err(|_| Error::FilesystemWriteFailure)?;
    //     path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
    //     path.extend_from_slice(&id.hex()).map_err(|_| Error::InternalError)?;
    //     Ok(path)
    // }

    // pub fn blob_path(&mut self, prefix: &Option<Letters>, id: Option<&UniqueId>)
    //     -> Result<Bytes<MAX_PATH_LENGTH>, Error> {
    //     let mut path = Bytes::<MAX_PATH_LENGTH>::new();

    //     path.extend_from_slice(&self.currently_serving).map_err(|_| Error::InternalError)?;
    //     path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;

    //     if let Some(prefix) = &prefix {
    //         if !prefix.0.iter().all(|b| *b >= b'a' && *b <= b'z') {
    //             return Err(crate::error::Error::NotJustLetters);
    //         }
    //         path.extend_from_slice(&prefix.0).map_err(|_| Error::InternalError)?;
    //         path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
    //     }

    //     // const HEX_CHARS: &[u8] = b"0123456789abcdef";
    //     // for byte in id.iter() {
    //     //     hprintln!("{}", &byte).ok();
    //     //     path.push(HEX_CHARS[(byte >> 4) as usize]).map_err(|_| Error::InternalError)?;
    //     //     path.push(HEX_CHARS[(byte & 0xf) as usize]).map_err(|_| Error::InternalError)?;
    //     // }
    //     if let Some(id) = id {
    //         path.extend_from_slice(&id.hex()).map_err(|_| Error::InternalError)?;
    //     }
    //     Ok(path)
    // }

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
        let resources = &mut self.resources;

        for ep in eps.iter_mut() {
            if !ep.send.ready() {
                continue;
            }
            if let Some(request) = ep.recv.dequeue() {
                // #[cfg(test)] println!("service got request: {:?}", &request);

                resources.currently_serving = ep.client_id.clone();
                let reply_result = resources.reply_to(request);
                ep.send.enqueue(reply_result).ok();

            }
        }
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
