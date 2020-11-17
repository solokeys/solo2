use core::convert::{TryFrom, TryInto};

use crate::logger::{info, blocking};
pub use embedded_hal::blocking::rng::Read as RngRead;
use heapless::ByteBuf;
use interchange::Responder;
use littlefs2::path::{Path, PathBuf};


use crate::api::*;
use crate::board::*;
use crate::config::*;
use crate::error::Error;
use crate::mechanisms;
use crate::pipe::TrussedInterchange;
use crate::store::{self, *};
use crate::types::*;

pub use crate::pipe::ServiceEndpoint;

// #[macro_use]
// mod macros;

macro_rules! rpc_trait { ($($Name:ident, $name:ident,)*) => { $(

    pub trait $Name<B: Board> {
        fn $name(_resources: &mut ServiceResources<B>, _request: request::$Name)
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
    UnsafeInjectKey, unsafe_inject_key,
    UnwrapKey, unwrap_key,
    Verify, verify,
    // TODO: can the default implementation be implemented in terms of Encrypt?
    WrapKey, wrap_key,
}

// associated keys end up namespaced under "/fido2"
// example: "/fido2/keys/2347234"
// let (mut fido_endpoint, mut fido2_client) = Client::new("fido2");
// let (mut piv_endpoint, mut piv_client) = Client::new("piv");

#[derive(Clone)]
struct ReadDirFilesState {
    request: request::ReadDirFilesFirst,
    last: PathBuf,
}

#[derive(Clone)]
struct ReadDirState {
    request: request::ReadDirFirst,
    last: usize,
}

pub struct ServiceResources<B>
where B: Board
{
    pub(crate) board: B,
    // Option?
    currently_serving: ClientId,
    // TODO: how/when to clear
    read_dir_files_state: Option<ReadDirFilesState>,
    read_dir_state: Option<ReadDirState>,
}

impl<B: Board> ServiceResources<B> {

    pub fn new(board: B) -> Self {
        Self {
            board,
            currently_serving: PathBuf::new(),
            read_dir_files_state: None,
            read_dir_state: None,
        }
    }
}

pub struct Service<B> where B: Board {
    eps: Vec<ServiceEndpoint, MAX_SERVICE_CLIENTS>,
    resources: ServiceResources<B>,
}

// need to be able to send crypto service to an interrupt handler
unsafe impl<B: Board> Send for Service<B> {}

impl<B: Board> ServiceResources<B> {

    pub fn reply_to(&mut self, request: Request) -> Result<Reply, Error> {
        // TODO: what we want to do here is map an enum to a generic type
        // Is there a nicer way to do this?
        // blocking::info!("trussed request: {:?}", &request).ok();
        // blocking::info!("IFS/EFS/VFS available BEFORE: {}/{}/{}",
        //       self.board.store().ifs().available_blocks().unwrap(),
        //       self.board.store().efs().available_blocks().unwrap(),
        //       self.board.store().vfs().available_blocks().unwrap(),
        // ).ok();
        blocking::debug!("trussed request: {:?}", &request).ok();
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
                    Mechanism::Tdes => mechanisms::Tdes::decrypt(self, request),
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
                    Mechanism::Tdes => mechanisms::Tdes::encrypt(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Encrypt(reply))
            },

            Request::Delete(request) => {
                // let success = store::delete_anywhere(&request.key.object_id);
                let key_types = [
                    KeyType::Secret,
                    KeyType::Public,
                ];

                let locations = [
                    StorageLocation::Internal,
                    StorageLocation::External,
                    StorageLocation::Volatile,
                ];

                let success = key_types.iter().any(|key_type| {
                    let path = self.key_path(*key_type, &request.key.object_id);
                    locations.iter().any(|location| {
                        store::delete(self.board.store(), *location, &path)
                    })
                });

                Ok(Reply::Delete(reply::Delete { success } ))
            },

            Request::Exists(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::exists(self, request),
                    Mechanism::P256 => mechanisms::P256::exists(self, request),
                    Mechanism::Totp => mechanisms::Totp::exists(self, request),
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

            Request::UnsafeInjectKey(request) => {
                match request.mechanism {
                    Mechanism::Tdes => mechanisms::Tdes::unsafe_inject_key(self, request),
                    Mechanism::Totp => mechanisms::Totp::unsafe_inject_key(self, request),
                    _ => Err(Error::MechanismNotAvailable),
                }.map(|reply| Reply::UnsafeInjectKey(reply))
            },

            Request::Hash(request) => {
                match request.mechanism {

                    Mechanism::Sha256 => mechanisms::Sha256::hash(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Hash(reply))
            },

            Request::LocateFile(request) => {

                let user_dir = match request.dir {
                    Some(dir) => dir,
                    None => PathBuf::from(b"/"),
                };
                let base_path = self.dataspace_path(&user_dir);
                let base_path = self.namespace_path(&base_path);
                blocking::info!("base path {:?}", &base_path).ok();

                fn recursively_locate<S: 'static + crate::types::LfsStorage>(
                    fs: &'static crate::store::Fs<S>,
                    path: PathBuf,
                    filename: &Path
                )
                    -> Result<Option<PathBuf>, littlefs2::io::Error>
                {
                    // blocking::info!("entering `rec-loc` with path {:?} and filename {:?}",
                              // &path, filename).ok();
                    // let fs = store.vfs();
                    fs.read_dir_and_then(&path, |dir| {
                        // blocking::info!("looking in {:?}", &path).ok();
                        for (i, entry) in dir.enumerate() {
                            let entry = entry.unwrap();
                            let mut is_special_dir = PathBuf::from(entry.file_name()) == PathBuf::from(b".");
                            is_special_dir |= PathBuf::from(entry.file_name()) == PathBuf::from(b"..");
                            if (i < 2) != is_special_dir {
                                // blocking::info!("i = {}, is_special_dir = {:?}", i, is_special_dir).ok();
                                panic!("i = {}, is_special_dir = {:?}, filename = {:?}",
                                    i,
                                    is_special_dir,
                                    entry.file_name(),
                                );

                            }
                            if i < 2 {
                                // blocking::info!(":: skipping {:?}", &entry.path()).ok();
                                continue;
                            }
                            if entry.file_type().is_file() {
                                // blocking::info!(":: comparing with {:?}", entry.file_name()).ok();
                                if PathBuf::from(entry.file_name()) == PathBuf::from(filename) {
                                    blocking::info!("found it").ok();
                                    return Ok(Some(PathBuf::from(entry.path())));
                                }
                                continue;
                            }
                            if entry.file_type().is_dir() {
                                // blocking::info!("recursing into {:?} with path {:?}",
                                //           &entry.file_name(),
                                //           &entry.path(),
                                //           ).ok();
                                if let Some(path) = recursively_locate(fs, PathBuf::from(entry.path()), filename)? {
                                    return Ok(Some(path));
                                };
                            }
                        }
                        Ok(None)
                    })
                }

                assert!(request.location == StorageLocation::Internal);
                let path = recursively_locate(self.board.store().ifs(), base_path, &request.filename).unwrap();
                let path = match path.as_ref() {
                    Some(path) => Some(self.denamedataspace_path(path)),
                    None => None,
                };
                    // .map_err(|_| Error::InternalError)?;

                Ok(Reply::LocateFile(reply::LocateFile { path }) )

            }

            Request::DebugDumpStore(_request) => {

                blocking::info!(":: PERSISTENT").ok();
                recursively_list(self.board.store().ifs(), PathBuf::from(b"/"));

                blocking::info!(":: VOLATILE").ok();
                recursively_list(self.board.store().vfs(), PathBuf::from(b"/"));

                fn recursively_list<S: 'static + crate::types::LfsStorage>(fs: &'static crate::store::Fs<S>, path: PathBuf) {
                    // let fs = store.vfs();
                    fs.read_dir_and_then(&path, |dir| {
                        for (i, entry) in dir.enumerate() {
                            let entry = entry.unwrap();
                            if i < 2 {
                                // blocking::info!("skipping {:?}", &entry.path()).ok();
                                continue;
                            }
                            blocking::info!("{:?} p({:?})", entry.path(), &path).ok();
                            if entry.file_type().is_dir() {
                                recursively_list(fs, PathBuf::from(entry.path()));
                            }
                            if entry.file_type().is_file() {
                                let _contents: Vec<u8, consts::U256> = fs.read(entry.path()).unwrap();
                                // blocking::info!("{} ?= {}", entry.metadata().len(), contents.len()).ok();
                                // blocking::info!("{:?}", &contents).ok();
                            }
                        }
                        Ok(())
                    }).unwrap();
                }

                Ok(Reply::DebugDumpStore(reply::DebugDumpStore {}) )

            }

            Request::ReadDirFirst(request) => {
                assert!(request.location == StorageLocation::Internal);

                let path = self.dataspace_path(&request.dir);
                let path = self.namespace_path(&path);
                let fs = self.board.store().ifs();

                let mut found_not_before = request.not_before_filename.is_none();
                let outcome = fs.read_dir_and_then(&path, |dir| {
                    for (i, entry) in dir.enumerate() {
                        if i < 2 {
                            continue;
                        }

                        let entry = entry.unwrap();
                        if found_not_before {
                            return Ok((i, entry));
                        } else {
                            found_not_before =
                                entry.file_name() ==
                                    request
                                        .not_before_filename.as_ref()
                                        .unwrap().as_ref()
                            ;
                            continue;
                        }
                    }

                    Err(littlefs2::io::Error::Io)
                });

                // we want an option, really
                // but let's abuse a result instead
                let maybe_entry = match outcome {
                    Ok((i, mut entry)) => {
                        self.read_dir_state = Some(ReadDirState {
                            request,
                            last: i,
                        });
                        *unsafe { entry.path_buf_mut() } = self.denamedataspace_path(&entry.path());
                        Some(entry)
                    }

                    Err(_) => {
                        self.read_dir_files_state = None;
                        None
                    }
                };
                Ok(Reply::ReadDirFirst(reply::ReadDirFirst {
                    entry: maybe_entry,
                } ))

            }

            Request::ReadDirNext(_request) => {
                let ReadDirState { request, last } = match &self.read_dir_state {
                    Some(state) => state.clone(),
                    None => panic!("call ReadDirFirst before ReadDirNext"),
                };

                assert!(request.location == StorageLocation::Internal);

                // let path = self.namespace_path(&request.dir);
                let path = self.dataspace_path(&request.dir);
                let path = self.namespace_path(&path);
                let fs = self.board.store().ifs();

                // let (i, entry) = fs.read_dir_and_then(&path, |dir| {
                let outcome = fs.read_dir_and_then(&path, |dir| {
                    for (i, entry) in dir.enumerate() {
                        if i <= last {
                            continue;
                        }

                        let entry = entry.unwrap();
                        return Ok((i, entry));
                    }

                    Err(littlefs2::io::Error::Io)
                });

                let maybe_entry = match outcome {
                    Ok((i, mut entry)) => {
                        self.read_dir_state = Some(ReadDirState {
                            request,
                            last: i,
                        });
                        *unsafe { entry.path_buf_mut() } = self.denamedataspace_path(&entry.path());
                        Some(entry)
                    }

                    Err(_) => {
                        self.read_dir_state = None;
                        None
                    }
                };
                Ok(Reply::ReadDirNext(reply::ReadDirNext {
                    entry: maybe_entry,
                } ))

            }

            Request::ReadDirFilesFirst(request) => {
                assert!(request.location == StorageLocation::Internal);

                // let path = self.namespace_path(&request.dir);
                let path = self.dataspace_path(&request.dir);
                let path = self.namespace_path(&path);

                let fs = self.board.store().ifs();

                let result = fs.read_dir_and_then(&path, |dir| {
                    for entry in dir {
                        // let entry = entry?;//.map_err(|_| Error::InternalError)?;
                        let entry = entry.unwrap();
                        if entry.file_type().is_dir() {
                            continue;
                        }

                        let name = entry.file_name();

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
                        }

                        return Ok(entry);
                    }

                    Err(littlefs2::io::Error::NoSuchEntry)
                });
                let entry = if result.is_err() {
                    let err = result.err().unwrap();
                    blocking::info!("read_dir error: {:?}", &err).ok();
                    return match err {
                        // Return no data if path is invalid
                        littlefs2::io::Error::NoSuchEntry =>
                            Ok(Reply::ReadDirFilesFirst(reply::ReadDirFilesFirst {
                                data: None,
                            } )),

                        _ => Err(Error::InternalError),
                    };
                } else {
                    result.unwrap()
                };

                let data = store::read(self.board.store(), request.location, entry.path())?;

                self.read_dir_files_state = Some(ReadDirFilesState {
                    request,
                    last: entry.file_name().into(),
                });

                Ok(Reply::ReadDirFilesFirst(reply::ReadDirFilesFirst {
                    data: Some(data),
                } ))
            }

            Request::ReadDirFilesNext(_request) => {
                // TODO: ergonooomics

                let ReadDirFilesState { request, last } = match &self.read_dir_files_state {
                    Some(state) => state.clone(),
                    None => panic!("call ReadDirFilesFirst before ReadDirFilesNext"),
                };

                // let path = self.namespace_path(&request.dir);
                let path = self.dataspace_path(&request.dir);
                let path = self.namespace_path(&path);
                let fs = self.board.store().ifs();

                let mut found_last = false;
                let entry = fs.read_dir_and_then(&path, |dir| {
                    for entry in dir {

                        let entry = entry.unwrap();

                        if entry.file_type().is_dir() {
                            continue;
                        }

                        let name = entry.file_name();

                        if !found_last {
                            let name: PathBuf = name.into();
                            // blocking::info!("comparing {:} with last {:?}", &name, &last).ok();
                            // TODO: This failed when all bytes (including trailing null) were
                            // compared. It turned out that `last` had a trailing 240 instead.
                            if last == name {
                                found_last = true;
                                // blocking::info!("found last").ok();
                            }
                            continue;
                        }

                        // blocking::info!("next file found: {:?}", name.as_ref()).ok();

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
                        }

                        return Ok(entry);
                    }

                    Err(littlefs2::io::Error::NoSuchEntry)

                });

                let data = match entry {
                    Err(littlefs2::io::Error::NoSuchEntry) => None,
                    Ok(entry) => {
                        let data = store::read(self.board.store(), request.location, entry.path())?;

                        self.read_dir_files_state = Some(ReadDirFilesState {
                            request,
                            last: entry.file_name().into(),
                        });

                        Some(data)

                    }
                    Err(_) => return Err(Error::InternalError),
                };

                Ok(Reply::ReadDirFilesNext(reply::ReadDirFilesNext {
                    data,
                } ))
            }

            Request::RemoveDir(request) => {
                // let path = self.blob_path(&request.path, Some(&request.id.object_id))?;
                // let path = self.namespace_path(&request.path);
                let path = self.dataspace_path(&request.path);
                let path = self.namespace_path(&path);
                let mut data = Message::new();
                data.resize_to_capacity();
                match request.location {
                    StorageLocation::Internal => self.board.store().ifs().remove_dir(&path),
                    StorageLocation::External => self.board.store().efs().remove_dir(&path),
                    StorageLocation::Volatile => self.board.store().vfs().remove_dir(&path),
                }.map_err(|_| Error::InternalError)?;
                // data.resize_default(size).map_err(|_| Error::InternalError)?;
                Ok(Reply::RemoveDir(reply::RemoveDir {} ))
            }

            Request::RemoveFile(request) => {
                // let path = self.blob_path(&request.path, Some(&request.id.object_id))?;
                // let path = self.namespace_path(&request.path);
                let path = self.dataspace_path(&request.path);
                let path = self.namespace_path(&path);
                let mut data = Message::new();
                data.resize_to_capacity();
                match request.location {
                    StorageLocation::Internal => self.board.store().ifs().remove(&path),
                    StorageLocation::External => self.board.store().efs().remove(&path),
                    StorageLocation::Volatile => self.board.store().vfs().remove(&path),
                }.map_err(|_| Error::InternalError)?;
                // data.resize_default(size).map_err(|_| Error::InternalError)?;
                Ok(Reply::RemoveFile(reply::RemoveFile {} ))
            }

            Request::ReadFile(request) => {
                // let path = self.blob_path(&request.path, Some(&request.id.object_id))?;
                let path = self.dataspace_path(&request.path);
                let path = self.namespace_path(&path);
                let mut data = Message::new();
                data.resize_to_capacity();
                let data: Message = match request.location {
                    StorageLocation::Internal => self.board.store().ifs().read(&path),
                    StorageLocation::External => self.board.store().efs().read(&path),
                    StorageLocation::Volatile => self.board.store().vfs().read(&path),
                }.map_err(|_| Error::InternalError)?.into();
                // data.resize_default(size).map_err(|_| Error::InternalError)?;
                Ok(Reply::ReadFile(reply::ReadFile { data } ))
            }

            Request::RandomByteBuf(request) => {
                if request.count < 1024 {
                    let mut bytes = Message::new();
                    bytes.resize_default(request.count).unwrap();
                    self.board.rng().read(&mut bytes)
                        .map_err(|_| Error::EntropyMalfunction)?;
                    Ok(Reply::RandomByteBuf(reply::RandomByteBuf { bytes } ))
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
                    Mechanism::P256Prehashed => mechanisms::P256Prehashed::sign(self, request),
                    Mechanism::Totp => mechanisms::Totp::sign(self, request),
                    _ => Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Sign(reply))
            },

            Request::WriteFile(request) => {
                let path = self.dataspace_path(&request.path);
                let path = self.namespace_path(&path);
                info!("WriteFile of size {}", request.data.len()).ok();
                store::store(self.board.store(), request.location, &path, &request.data)?;
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

            Request::RequestUserConsent(request) => {
                assert_eq!(request.level, consent::Level::Normal);

                let starttime = self.board.user_interface().uptime();
                let timeout = core::time::Duration::from_millis(request.timeout_milliseconds as u64);

                self.board.user_interface().set_status(ui::Status::WaitingForUserPresence);
                loop {
                    let nowtime = self.board.user_interface().uptime();
                    if (nowtime - starttime) > timeout {
                        let result = Err(consent::Error::TimedOut);
                        return Ok(Reply::RequestUserConsent(reply::RequestUserConsent { result } ));
                    }
                    let up = self.board.user_interface().check_user_presence();
                    match request.level {
                        // If Normal level consent is request, then both Strong and Normal
                        // indications will result in success.
                        consent::Level::Normal => {
                            if up == consent::Level::Normal ||
                                up == consent::Level::Strong {
                                    break;
                                }
                        },
                        // Otherwise, only strong level indication will work.
                        consent::Level::Strong => {
                            if up == consent::Level::Strong {
                                break;
                            }
                        }
                        _ => {
                            break;
                        }
                    }
                }
                self.board.user_interface().set_status(ui::Status::Idle);

                let result = Ok(());
                Ok(Reply::RequestUserConsent(reply::RequestUserConsent { result } ))
            }

            Request::Reboot(request) => {
                self.board.user_interface().reboot(request.to);
            }

            _ => {
                // #[cfg(test)]
                // println!("todo: {:?} request!", &request);
                Err(Error::RequestNotAvailable)
            },
        }
    }

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

    pub fn dataspace_path(&self, path: &Path) -> PathBuf {
        // TODO: check no escapes!
        let mut dataspaced_path = PathBuf::new();
        dataspaced_path.push(b"dat\0".try_into().unwrap());
        dataspaced_path.push(path);
        dataspaced_path
    }

    pub fn denamespace_path(&self, path: &Path) -> PathBuf {
        // blocking::info!("denamespacing {:?}", path).ok();
        let bytes = path.as_ref().as_bytes();
        let absolute = bytes[0] == b'/';
        let offset = if absolute { 1 } else { 0 };

        let end_of_namespace = bytes[1..].iter().position(|&x| x == b'/')
            // oh oh oh
            .unwrap();
        let buf = PathBuf::from(&bytes[end_of_namespace + 1 + offset..]);
        // blocking::info!("buf out: {:?}", &buf).ok();
        buf
    }

    pub fn dedataspace_path(&self, path: &Path) -> PathBuf {
        // blocking::info!("dedataspacing {:?}", path).ok();
        let bytes = path.as_ref().as_bytes();
        let absolute = bytes[0] == b'/';
        let offset = if absolute { 1 } else { 0 };

        let end_of_dataspace = bytes[1..].iter().position(|&x| x == b'/')
            // oh oh oh
            .unwrap();
        let buf = PathBuf::from(&bytes[end_of_dataspace + 1 + offset..]);
        // blocking::info!("buf out: {:?}", &buf).ok();
        buf
    }

    pub fn denamedataspace_path(&self, path: &Path) -> PathBuf {
        self.dedataspace_path(&self.denamespace_path(path))
    }

    pub fn key_path(&self, key_type: KeyType, key_id: &UniqueId) -> PathBuf {
        let mut path = PathBuf::new();
        // TODO: huh?!?!
        // If I change these prefixes to shorter,
        // DebugDumpStore skips the directory contents
        path.push(match key_type {
            KeyType::Public => b"pub\0".try_into().unwrap(),
            KeyType::Secret => b"sec\0".try_into().unwrap(),
        });
        path.push(&PathBuf::from(&key_id.hex()));
        // no dataspacing
        self.namespace_path(&path)
    }

    pub fn store_key(&mut self, location: StorageLocation, key_type: KeyType, key_kind: KeyKind, key_material: &[u8]) -> Result<UniqueId, Error> {
        blocking::info!("STORING {:?} -> {:?}", &key_kind, location).ok();
        let serialized_key = SerializedKey::try_from((key_kind, key_material))?;

        let mut buf = [0u8; 128];
        let serialized_bytes = crate::cbor_serialize(&serialized_key, &mut buf).map_err(|_| Error::CborError)?;
        let key_id = self.generate_unique_id()?;
        let path = self.key_path(key_type, &key_id);

        store::store(self.board.store(), location, &path, &serialized_bytes)?;

        Ok(key_id)
    }

    pub fn overwrite_key(&self, location: StorageLocation, key_type: KeyType, key_kind: KeyKind, key_id: &UniqueId, key_material: &[u8]) -> Result<(), Error> {
        let serialized_key = SerializedKey::try_from((key_kind, key_material))?;

        let mut buf = [0u8; 128];
        let serialized_bytes = crate::cbor_serialize(&serialized_key, &mut buf).map_err(|_| Error::CborError)?;

        let path = self.key_path(key_type, key_id);

        store::store(self.board.store(), location, &path, &serialized_bytes)?;

        Ok(())
    }

    pub fn key_id_location(&self, key_type: KeyType, key_id: &UniqueId) -> Option<StorageLocation> {
        let path = self.key_path(key_type, key_id);

        if path.exists(&self.board.store().vfs()) {
            return Some(StorageLocation::Volatile);
        }

        if path.exists(&self.board.store().ifs()) {
            return Some(StorageLocation::Internal);
        }

        if path.exists(&self.board.store().efs()) {
            return Some(StorageLocation::External);
        }

        None
    }

    pub fn exists_key(&self, key_type: KeyType, key_kind: Option<KeyKind>, key_id: &UniqueId)
        -> bool  {
        self.load_key(key_type, key_kind, key_id).is_ok()
    }

    pub fn load_key(&self, key_type: KeyType, key_kind: Option<KeyKind>, key_id: &UniqueId)
        -> Result<SerializedKey, Error>  {

        // blocking::info!("LOADING {:?}", &key_kind).ok();
        let path = self.key_path(key_type, key_id);

        let location = match self.key_id_location(key_type, key_id) {
            Some(location) => location,
            None => return Err(Error::NoSuchKey),
        };

        let bytes: ByteBuf<consts::U128> = store::read(self.board.store(), location, &path)?;

        let serialized_key: SerializedKey = crate::cbor_deserialize(&bytes).map_err(|_| Error::CborError)?;

        if let Some(kind) = key_kind {
            if serialized_key.kind != kind {
                blocking::info!("wrong key kind, expected {:?} got {:?}", &kind, &serialized_key.kind).ok();
                Err(Error::WrongKeyKind)?;
            }
        }
        Ok(serialized_key)
    }

    pub fn generate_unique_id(&mut self) -> Result<UniqueId, Error> {
        let mut unique_id = [0u8; 16];

        self.board.rng().read(&mut unique_id)
            .map_err(|_| Error::EntropyMalfunction)?;

        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("unique id {:?}", &unique_id);
        Ok(UniqueId(unique_id))
    }

}

impl<B: Board> Service<B> {

    pub fn new(board: B) -> Self {
        let resources = ServiceResources::new(board);
        Self { eps: Vec::new(), resources }
    }

    pub fn add_endpoint(&mut self, interchange: Responder<TrussedInterchange>, client_id: ClientId) -> Result<(), ServiceEndpoint> {
        self.eps.push(ServiceEndpoint { interchange, client_id })
    }

    // currently, this just blinks the green heartbeat LED (former toggle_red in app_rtic.rs)
    //
    // in future, this would
    // - generate more interesting LED visuals
    // - return "when" next to be called
    // - potentially read out button status and return "async"
    pub fn update_ui(&mut self) /* -> u32 */ {
        self.resources.board.user_interface().refresh();
    }

    // process one request per client which has any
    pub fn process(&mut self) {
        // split self since we iter-mut over eps and need &mut of the other resources
        let eps = &mut self.eps;
        let resources = &mut self.resources;

        for ep in eps.iter_mut() {
            if let Some(request) = ep.interchange.take_request() {
                // #[cfg(test)] println!("service got request: {:?}", &request);

                resources.currently_serving = ep.client_id.clone();
                let reply_result = resources.reply_to(request);
                ep.interchange.respond(reply_result).ok();

            }
        }
        blocking::debug!("I/E/V : {}/{}/{} >",
              self.resources.board.store().ifs().available_blocks().unwrap(),
              self.resources.board.store().efs().available_blocks().unwrap(),
              self.resources.board.store().vfs().available_blocks().unwrap(),
        ).ok();
    }
}

impl<B> crate::pipe::Syscall for &mut Service<B>
where B: Board
{
    fn syscall(&mut self) {
        self.process();
    }
}
