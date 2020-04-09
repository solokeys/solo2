use core::convert::TryFrom;

#[cfg(feature = "semihosting")]
use cortex_m_semihosting::hprintln;
use heapless_bytes::Bytes;
use serde_indexed::{DeserializeIndexed, SerializeIndexed};

use crate::api::*;
use crate::config::*;
use crate::error::Error;
use crate::mechanisms;
use crate::types::*;

pub use crate::pipe::ServiceEndpoint;

pub use embedded_hal::blocking::rng::Read as RngRead;

// #[macro_use]
// mod macros;

pub trait Agree<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn agree(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::Agree)
    -> Result<reply::Agree, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Decrypt<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn decrypt(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::Decrypt)
    -> Result<reply::Decrypt, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait DeriveKey<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn derive_key(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::DeriveKey)
    -> Result<reply::DeriveKey, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait DeserializeKey<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn deserialize_key(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::DeserializeKey)
    -> Result<reply::DeserializeKey, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Encrypt<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn encrypt(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::Encrypt)
    -> Result<reply::Encrypt, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Exists<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn exists(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::Exists)
    -> Result<reply::Exists, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait GenerateKey<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn generate_key(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::GenerateKey)
    -> Result<reply::GenerateKey, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Hash<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn hash(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::Hash)
    -> Result<reply::Hash, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait SerializeKey<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn serialize_key(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::SerializeKey)
    -> Result<reply::SerializeKey, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Sign<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn sign(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::Sign)
    -> Result<reply::Sign, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait UnwrapKey<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn unwrap_key(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::UnwrapKey)
    -> Result<reply::UnwrapKey, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Verify<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn verify(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::Verify)
    -> Result<reply::Verify, Error> { Err(Error::MechanismNotAvailable) }
}

// TODO: can the default implementation be implemented in terms of Encrypt?
pub trait WrapKey<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    fn wrap_key(_resources: &mut ServiceResources<'a, 's, R, I, E, V>, _request: request::WrapKey)
    -> Result<reply::WrapKey, Error> { Err(Error::MechanismNotAvailable) }
}

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
// #[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
// #[serde(rename_all = "camelCase")]
// #[serde_indexed(offset = 1)]
pub struct SerializedKey {
   // r#type: KeyType,
   pub kind: KeyKind,
   pub value: Bytes<MAX_SERIALIZED_KEY_LENGTH>,
}

impl<'a> TryFrom<(KeyKind, &'a [u8])> for SerializedKey {
    type Error = Error;
    fn try_from(from: (KeyKind, &'a [u8])) -> Result<Self, Error> {
        Ok(SerializedKey {
            kind: from.0,
            value: Bytes::try_from_slice(from.1).map_err(|_| Error::InternalError)?,
        })
    }
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

pub struct TriStorage<'s, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    /// internal FLASH storage
    ifs: FilesystemWith<'s, 's, I>,
    /// external FLASH storage
    efs: FilesystemWith<'s, 's, E>,
    /// volatile RAM storage
    vfs: FilesystemWith<'s, 's, V>,
}

pub struct ServiceResources<'a, 's, R, I, E, V>
where
    R: RngRead,
    I: LfsStorage,
    E: LfsStorage,
    V: LfsStorage,
{
    pub(crate) rng: R,
    pub(crate) tri: TriStorage<'s, I, E, V>,
    currently_serving: &'a str,
}

pub(crate) fn load_serialized_key<'s, S: LfsStorage>(fs: &mut FilesystemWith<'s, 's, S>, path: &[u8], buf: &mut [u8]) -> Result<usize, Error> {

    use littlefs2::fs::{File, FileWith};
    use littlefs2::io::ReadWith;

    let mut alloc = File::allocate();
    // hprintln!("sizeof<FileAllocation> = {}", core::mem::size_of::<littlefs2::fs::FileAllocation<S>>()).ok();
    // hprintln!("sizeof<FilesystemAllocation> = {}", core::mem::size_of::<littlefs2::fs::FilesystemAllocation<S>>()).ok();
    // hprintln!("sizeof<lfs_t> = {}", core::mem::size_of::<littlefs2::fs::ll::lfs_t>()).ok();
    // hprintln!("sizeof<lfs_config> = {}", core::mem::size_of::<littlefs2::fs::ll::lfs_config>()).ok();
    // hprintln!("sizeof<lfs_file_t> = {}", core::mem::size_of::<littlefs2::fs::ll::lfs_file_t>()).ok();
    // hprintln!("sizeof<lfs_file_config> = {}", core::mem::size_of::<littlefs2::fs::ll::lfs_file_config>()).ok();
    // hprintln!("opening path = {:?}", &path[..]).ok();
    let mut file = FileWith::open(&path[..], &mut alloc, fs)
        .map_err(|_| Error::FilesystemReadFailure)?;

    // hprintln!("reading it").ok();
    let size = file.read(buf)
        .map_err(|_| Error::FilesystemReadFailure)?;

    Ok(size)
}

// pub fn find_next(
//     fs: &mut FilesystemWith<'s, 's, S>,
//     dir: &[u8],
//     user_attribute: Option<UserAttribute>,
//     previous: Option<ObjectHandle>,
// )
//     -> Result<Option<ObjectHandle>, Error>
// {
//     let mut read_dir = fs.read_dir(dir, &mut storage).unwrap();
// }

pub fn create_directories<'s, S: LfsStorage>(
    fs: &mut FilesystemWith<'s, 's, S>,
    path: &[u8],
) -> Result<(), Error>
{
    // hprintln!("preparing {:?}", core::str::from_utf8(path).unwrap()).ok();
    for i in 0..path.len() {
        if path[i] == b'/' {
            let dir = &path[..i];
            let dir_str = core::str::from_utf8(dir).unwrap();
            // hprintln!("create dir {:?}", dir_str).ok();
            // fs.create_dir(dir).map_err(|_| Error::FilesystemWriteFailure)?;
            match fs.create_dir(dir) {
                Err(littlefs2::io::Error::EntryAlreadyExisted) => {}
                Ok(()) => {}
                error => { panic!("{:?}", &error); }
            }
        }
    }
    Ok(())
}

pub fn store_serialized_key<'s, S: LfsStorage>(
    fs: &mut FilesystemWith<'s, 's, S>,
    path: &[u8], buf: &[u8],
    user_attribute: Option<UserAttribute>,
)
    -> Result<(), Error>
{
    use littlefs2::fs::{Attribute, File, FileWith};

    // create directories if missing
    create_directories(fs, path)?;

    let mut alloc = File::allocate();
    {
        let mut file = FileWith::create(&path[..], &mut alloc, fs)
            .map_err(|_| Error::FilesystemWriteFailure)?;
        use littlefs2::io::WriteWith;
        file.write(&buf)
            .map_err(|_| Error::FilesystemWriteFailure)?;
        file.sync()
            .map_err(|_| Error::FilesystemWriteFailure)?;
    }

    if let Some(user_attribute) = user_attribute.as_ref() {
        let mut attribute = Attribute::new(crate::config::USER_ATTRIBUTE_NUMBER);
        attribute.set_data(user_attribute);
        fs.set_attribute(path, &attribute).map_err(|e| {
            info!("error setting attribute: {:?}", &e).ok();
            Error::FilesystemWriteFailure
        })?;
    }

    // file.close()
    //     .map_err(|_| Error::FilesystemWriteFailure)?;
    // #[cfg(test)]
    // println!("closed file");

    Ok(())
}

pub(crate) fn delete<'s, S: LfsStorage>(fs: &mut FilesystemWith<'s, 's, S>, path: &[u8]) -> bool {

    match fs.remove(path) {
        Ok(_) => true,
        Err(_) => false,
    }
}

impl<'s, I: LfsStorage, E: LfsStorage, V: LfsStorage> TriStorage<'s, I, E, V> {

    pub fn delete_key(&mut self, path: &[u8]) -> bool {

        // try each storage backend in turn, attempting to locate the key
        match delete(&mut self.vfs, path) {
            true => true,
            false => {
                match delete(&mut self.ifs, path) {
                    true => true,
                    false => {
                        delete(&mut self.efs, path)
                    }
                }
            }
        }
    }

    pub fn load_key_unchecked(&mut self, path: &[u8]) -> Result<(SerializedKey, StorageLocation), Error> {
        // #[cfg(test)]
        // // actually safe, as path is ASCII by construction
        // println!("loading from file {:?}", unsafe { core::str::from_utf8_unchecked(&path[..]) });

        let mut buf = [0u8; 128];

        // try each storage backend in turn, attempting to locate the key
        let location = match load_serialized_key(&mut self.vfs, path, &mut buf) {
            Ok(_) => StorageLocation::Volatile,
            Err(_) => {
                match load_serialized_key(&mut self.ifs, path, &mut buf) {
                    Ok(_) => StorageLocation::Internal,
                    Err(_) => {
                        match load_serialized_key(&mut self.efs, path, &mut buf) {
                            Ok(_) => StorageLocation::External,
                            Err(_) => return Err(Error::NoSuchKey),
                        }
                    }
                }
            }
        };

        let serialized_key: SerializedKey = crate::cbor_deserialize(&buf).map_err(|_| Error::CborError)?;
        Ok((serialized_key, location))

    }

    pub fn load_key(&mut self, path: &[u8], kind: KeyKind, key_bytes: &mut [u8]) -> Result<StorageLocation, Error> {
        // #[cfg(test)]
        // // actually safe, as path is ASCII by construction
        // println!("loading from file {:?}", unsafe { core::str::from_utf8_unchecked(&path[..]) });

        let (serialized_key, location) = self.load_key_unchecked(path)?;
        if serialized_key.kind != kind {
            hprintln!("wrong key kind, expected {:?} got {:?}", &kind, &serialized_key.kind).ok();
            Err(Error::WrongKeyKind)?;
        }

        key_bytes.copy_from_slice(&serialized_key.value);
        Ok(location)
    }

    // TODO: in the case of desktop/ram storage:
    // - using file.sync (without file.close) leads to an endless loop
    // - this loop happens inside `lfs_dir_commit`, namely inside its first for loop
    //   https://github.com/ARMmbed/littlefs/blob/v2.1.4/lfs.c#L1680-L1694
    // - the `if` condition is never fulfilled, it seems f->next continues "forever"
    //   through whatever lfs->mlist is.
    //
    // see also https://github.com/ARMmbed/littlefs/issues/145
    //
    // OUTCOME: either ensure calling `.close()`, or patch the call in a `drop` for FileWith.
    //
    pub fn store_key(&mut self, persistence: StorageLocation, path: &[u8], kind: KeyKind, key_bytes: &[u8]) -> Result<(), Error> {
        // actually safe, as path is ASCII by construction
        // #[cfg(test)]
        // println!("storing in file {:?}", unsafe { core::str::from_utf8_unchecked(&path[..]) });

        let serialized_key = SerializedKey::try_from((kind, key_bytes))?;
        let mut buf = [0u8; 128];
        crate::cbor_serialize(&serialized_key, &mut buf).map_err(|_| Error::CborError)?;

        match persistence {
            StorageLocation::Internal => store_serialized_key(&mut self.ifs, path, &buf, None),
            StorageLocation::External => store_serialized_key(&mut self.efs, path, &buf, None),
            StorageLocation::Volatile => store_serialized_key(&mut self.vfs, path, &buf, None),
        }

    }

}

pub struct Service<'a, 's, R, I, E, V>
where
    R: RngRead,
    I: LfsStorage,
    E: LfsStorage,
    V: LfsStorage,
{
    eps: Vec<ServiceEndpoint<'a>, MAX_SERVICE_CLIENTS>,
    resources: ServiceResources<'a, 's, R, I, E, V>,
}

// need to be able to send crypto service to an interrupt handler
unsafe impl<R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> Send for Service<'_, '_, R, I, E, V> {}

impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> ServiceResources<'a, 's, R, I, E, V> {

    pub fn try_new(
        rng: R,
        ifs: FilesystemWith<'s, 's, I>,
        efs: FilesystemWith<'s, 's, E>,
        vfs: FilesystemWith<'s, 's, V>,
    ) -> Result<Self, Error> {

        Ok(Self { rng, tri: TriStorage { ifs, efs, vfs }, currently_serving: &"" })
    }

    pub fn load_key_unchecked(&mut self, path: &[u8]) -> Result<(SerializedKey, StorageLocation), Error> {
        self.tri.load_key_unchecked(path)
    }

    pub fn load_key(&mut self, path: &[u8], kind: KeyKind, key_bytes: &mut [u8])
        -> Result<StorageLocation, Error>
    {
        self.tri.load_key(path, kind, key_bytes)
    }

    pub fn store_key(&mut self, to: StorageLocation, path: &[u8], kind: KeyKind, key_bytes: &[u8])
        -> Result<(), Error>
    {
        self.tri.store_key(to, path, kind, key_bytes)
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
              self.tri.ifs.available_blocks().unwrap(),
              self.tri.efs.available_blocks().unwrap(),
              self.tri.vfs.available_blocks().unwrap(),
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
                    match self.tri.delete_key(&path) {
                        true => true,
                        false => {
                            let path = self.prepare_path_for_key(KeyType::Public, &request.key.object_id)?;
                            match self.tri.delete_key(&path) {
                                true => true,
                                false => {
                                    let path = self.prepare_path_for_key(KeyType::Secret, &request.key.object_id)?;
                                    self.tri.delete_key(&path)
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
                let path = self.blob_path(&request.prefix, None)?;
                // TODO: ergonooomics

                let mut path = Bytes::<MAX_PATH_LENGTH>::new();
                path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
                path.extend_from_slice(self.currently_serving.as_bytes()).map_err(|_| Error::InternalError)?;
                if let Some(prefix) = request.prefix.clone() {
                    path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
                    path.extend_from_slice(&prefix.0).map_err(|_| Error::InternalError)?;
                }

                #[cfg(feature = "semihosting")]
                hprintln!("listing blobs in {:?}", &path).ok();

                let entry = self.tri.ifs.within(
                    |fs, storage| -> littlefs2::io::Result<littlefs2::fs::DirEntry<I>> {
                        let mut read_dir = fs.read_dir(&path[..], storage)?;
                        // "desugared iterator"
                        loop {
                            match read_dir.next(fs, storage) {
                                Some(entry) => {
                                    let entry = entry?;
                                    if entry.file_type().is_dir() {
                                        // no filesystem walking here
                                        let filename = entry.file_name();
                                        let filename_bytes = filename.as_bytes();
                                        let l = filename_bytes.into_iter().position(|x| *x == b'\0').unwrap();
                                        #[cfg(feature = "semihosting")]
                                        hprintln!("skipping subdirectory {:?}", core::str::from_utf8(&filename_bytes[..l]).unwrap()).ok();
                                        continue;
                                    }

                                    let filename = entry.file_name();
                                    let filename_bytes = filename.as_bytes();
                                    let l = filename_bytes.into_iter().position(|x| *x == b'\0').unwrap();
                                    #[cfg(feature = "semihosting")]
                                    hprintln!("first file found: {:?}", core::str::from_utf8(&filename_bytes[..l]).unwrap()).ok();

                                    // check user attribute
                                    if let Some(user_attribute) = request.user_attribute.as_ref() {
                                        let mut path = path.clone();
                                        path.extend_from_slice(b"/").map_err(|_| littlefs2::io::Error::NoMemory)?;
                                        #[cfg(feature = "semihosting")]
                                        path.extend_from_slice(&filename_bytes[..l]).map_err(|_| littlefs2::io::Error::NoMemory)?;

                                        let attribute = fs.attribute(&path[..], crate::config::USER_ATTRIBUTE_NUMBER, storage)
                                            .map_err(|e| {
                                                info!("error getting attribute: {:?}", &e).ok();
                                                littlefs2::io::Error::Io
                                        })?;

                                        match attribute {
                                            None => {
                                                #[cfg(feature = "semihosting")]
                                                hprintln!("user attribute requested, none attached").ok();
                                                continue;
                                            }
                                            Some(attribute) => {
                                                // #[cfg(feature = "semihosting")]
                                                // hprintln!("attribute requested: {:?}", user_attribute).ok();
                                                // #[cfg(feature = "semihosting")]
                                                // hprintln!("attribute present: {:?}", attribute.data()).ok();
                                                if user_attribute != attribute.data() {
                                                    #[cfg(feature = "semihosting")]
                                                    hprintln!("not equal").ok();
                                                    continue;
                                                }
                                            }
                                        }
                                    }

                                    return Ok(entry);
                                }
                                None => break,
                            }
                        }
                        Err(littlefs2::io::Error::NoSuchEntry)
                    }
                ).unwrap();//map_err(|_| Error::InternalError)?;

                // let unique_id = UniqueId::try_from_hex(&entry.file_name().as_bytes()).map_err(|_| Error::InternalError)?;
                let filename = entry.file_name();
                let filename_bytes = filename.as_bytes();
                let l = filename_bytes.into_iter().position(|x| *x == b'\0').unwrap();
                let filename_bytes = &filename_bytes[..l];
                hprintln!("filename bytes: {:?}", filename_bytes).ok();
                let unique_id = UniqueId::try_from_hex(filename_bytes).unwrap();
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
                let size = match request.location {
                    StorageLocation::Internal => load_serialized_key(&mut self.tri.ifs, &path, &mut data),
                    StorageLocation::External => load_serialized_key(&mut self.tri.efs, &path, &mut data),
                    StorageLocation::Volatile => load_serialized_key(&mut self.tri.vfs, &path, &mut data),
                }?;
                data.resize_default(size).map_err(|_| Error::InternalError)?;
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
                        &mut self.tri.ifs,& path, &request.data, request.user_attribute),
                    StorageLocation::External => store_serialized_key(
                        &mut self.tri.efs, &path, &request.data, request.user_attribute),
                    StorageLocation::Volatile => store_serialized_key(
                        &mut self.tri.vfs, &path, &request.data, request.user_attribute),
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
        path.extend_from_slice(self.currently_serving.as_bytes()).map_err(|_| Error::InternalError)?;
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

        path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
        path.extend_from_slice(self.currently_serving.as_bytes()).map_err(|_| Error::InternalError)?;
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

impl<'a, 's, R: RngRead, I: LfsStorage, E: LfsStorage, V: LfsStorage> Service<'a, 's, R, I, E, V> {

    pub fn new(
        rng: R,
        internal_storage: FilesystemWith<'s, 's, I>,
        external_storage: FilesystemWith<'s, 's, E>,
        volatile_storage: FilesystemWith<'s, 's, V>,
    )
        -> Result<Self, Error>
    {
        let resources = ServiceResources::try_new(rng, internal_storage, external_storage, volatile_storage)?;
        Ok(Self { eps: Vec::new(), resources, })
    }

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

                resources.currently_serving = &ep.client_id;
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
              self.resources.tri.ifs.available_blocks().unwrap(),
              self.resources.tri.efs.available_blocks().unwrap(),
              self.resources.tri.vfs.available_blocks().unwrap(),
        ).ok();
    }
}

impl<'a, 's, R, I, E, V> crate::pipe::Syscall for &mut Service<'a, 's, R, I, E, V>
where
    R: RngRead,
    I: LfsStorage,
    E: LfsStorage,
    V: LfsStorage,
{
    fn syscall(&mut self) {
        self.process();
    }
}
