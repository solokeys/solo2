//! `store` presents a combined interface to three littlefs2 filesystems:
//! internal flash, external flash, volatile/RAM.
//!
//! It covers two usecases:
//! - cryptographic key storage (for trussed itself)
//! - somewhat namespaced key-value storage for client apps
//!
//! The cryptographic keys are stored with a random filename (which is used as
//! "handle" for the key).
//!
//! The key-value storage has keys aka filenames choosable by the client.
//!
//! The guiding example for client apps is `fido-authenticator`, which stores:
//! - it basic state and config, and
//! - the metadata for its resident keys as a serialized struct
//! Both include references to cryptographic keys (via their handle)
//!
//! Currently, the backend (internal/external/volatile) is determined via an
//! enum parameter, which is translated to the corresponding generic type.
//! I think it would be nice to "mount" the three in a unified filesystem,
//! e.g. internal under `/`, external under `/mnt` (it's not available when
//! powered via NFC), volatile under `/tmp`.
//!
//! If this is done, it would be great to abstract over the three backends,
//! and just take some array with associated "mount points". But KISS it ofc...
//!
//! This store needs to enforce namespacing by apps, ensuring they can't escape
//! by loading some file `../../<other app>/keys/...` or similar.
//! This is orthogonal to the three backends split, I'm not quite sure yet how
//! to expose this and how to map this to paths.
//!
//!
//! Here's my current thinking:
//!
//! /
//! |-- data/
//!     |-- <app id>/
//!         |--dir.1/
//!            +-- file.1
//!            +-- file.2
//!         |-- dir.2
//!            +-- file.1
//!            +-- file.2
//!         +-- file.1
//!         +-- file.2
//! |-- keys/
//!
//! NOTE !!! ==> ideally can filter out CredentialProtectionPolicy in ReadDirFiles (via attribute)
//!
//! (fido)
//!     :   |-- data/              <-- the KeyValue portion
//!         :   |-- rk/
//!                 |-- <rp hash>/
//!                     + rk.1
//!                     + rk.2
//!                     :
//!                 |-- <rp hash>/
//!             + config
//!             +
//!
//! Why? This:
//! - typical use-case is one RK per RP (I'd assume!)
//! - allows easy lookup in this case!
//! - allows easy "count RKs" (possibly filtered) for GetAssertion
//! - allows easy "count RPs" (for CredMgmt)
//! - CON: this is already two directories deep (not just "one namespace')
//! - Alternative: subdirectory <==> RP hash, everything else in flat files
//! - In any case need to "list dirs excluding . and .." or similar

use core::convert::TryFrom;

#[allow(unused_imports)]
#[cfg(feature = "semihosting")]
use cortex_m_semihosting::hprintln;
use littlefs2::path::Path;
use serde_indexed::{DeserializeIndexed, SerializeIndexed};

use crate::config::*;
use crate::error::Error;
use crate::types::*;

// pub type FileContents = ByteBuf<MAX_FILE_SIZE>;

// pub mod our {
//     type Result = ();
// }

// pub trait KeyValue: Store + Copy {
//     fn set(
//         // "root" or an actual client. Maybe map to `/root` and `/home/<client>`?
//         client: ClientId,
//         // this needs to be piped via RPC, the idea is to allow a file "config"
//         // with no namespace, and e.g. a namespace "rk" that can easily be iterated over.
//         namespace: Option<PathComponent>,
//         // intention of attributes is to allow for easy (client-specified) filtering
//         // without reading and transmitting the contents of each file (it would be neat
//         // for the RPC call to pass a closure-filter, but I doubt would work currently).
//         // For instance, `fido-authenticator` can use "hashed RP ID" under its `rk` namespace.
//         attribute: Option<Attribute>,

//         // the data
//         data: FileContents,
//     ) -> our::Result<()>;

//     fn get(
//         client: ClientId,
//         namespace: Option<PathComponent>,
//         attribute: Option<Attribute>,
//     ) -> our::Result<FileContents>;
// }

// pub trait CryptoKey: Store + Copy {
// }

// This is a "trick" I learned from japaric's rewrite of the littlefs
// API, using a trait and a macro (that the caller implements with the specific
// LfsStorage-bound types) to remove lifetimes and generic parameters from Store.
//
// This makes everything using it *much* more ergonomic.
pub unsafe trait Store: Copy {
    type I: 'static + LfsStorage;
    type E: 'static + LfsStorage;
    type V: 'static + LfsStorage;
    fn ifs(self) -> &'static Fs<Self::I>;
    fn efs(self) -> &'static Fs<Self::E>;
    fn vfs(self) -> &'static Fs<Self::V>;
}

pub struct Fs<S: 'static + LfsStorage> {
    fs: &'static Filesystem<'static, S>,
}

impl<S: 'static + LfsStorage> core::ops::Deref for Fs<S> {
    type Target = Filesystem<'static, S>;
    fn deref(&self) -> &Self::Target {
        &self.fs
    }
}

impl<S: 'static + LfsStorage> Fs<S> {
    pub fn new(fs: &'static Filesystem<'static, S>) -> Self {
        Self { fs }
    }
}

#[macro_export]
macro_rules! store { (
    $store:ident,
    Internal: $Ifs:ty,
    External: $Efs:ty,
    Volatile: $Vfs:ty
) => {
    #[derive(Clone, Copy)]
    pub struct $store {
        // __: $crate::store::NotSendOrSync,
        __: core::marker::PhantomData<*mut ()>,
    }

    unsafe impl $crate::store::Store for $store {
        type I = $Ifs;
        type E = $Efs;
        type V = $Vfs;

        fn ifs(self) -> &'static $crate::store::Fs<$Ifs> {
            unsafe { &*Self::ifs_ptr() }
        }
        fn efs(self) -> &'static $crate::store::Fs<$Efs> {
            unsafe { &*Self::efs_ptr() }
        }
        fn vfs(self) -> &'static $crate::store::Fs<$Vfs> {
            unsafe { &*Self::vfs_ptr() }
        }
    }

    impl $store {
        pub fn claim() -> Option<$store> {
            use core::sync::atomic::{AtomicBool, Ordering};
            // use $crate::store::NotSendOrSync;

            static CLAIMED: AtomicBool = AtomicBool::new(false);

            if CLAIMED
                .compare_exchange_weak(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Some(Self { __: unsafe { $crate::store::NotSendOrSync::new() } })
                Some(Self { __: core::marker::PhantomData })
            } else {
                None
            }
        }

        fn ifs_ptr() -> *mut $crate::store::Fs<$Ifs> {
            use core::{mem::MaybeUninit};
            use $crate::store::Fs;
            static mut IFS: MaybeUninit<Fs<$Ifs>> = MaybeUninit::uninit();
            unsafe { IFS.as_mut_ptr() }
        }

        fn efs_ptr() -> *mut $crate::store::Fs<$Efs> {
            use core::{mem::MaybeUninit};
            use $crate::store::Fs;
            static mut EFS: MaybeUninit<Fs<$Efs>> = MaybeUninit::uninit();
            unsafe { EFS.as_mut_ptr() }
        }

        fn vfs_ptr() -> *mut $crate::store::Fs<$Vfs> {
            use core::{mem::MaybeUninit};
            use $crate::store::Fs;
            static mut VFS: MaybeUninit<Fs<$Vfs>> = MaybeUninit::uninit();
            unsafe { VFS.as_mut_ptr() }
        }

        pub fn mount(
            &self,
            ifs_alloc: &'static mut littlefs2::fs::Allocation<$Ifs>,
            ifs_storage: &'static mut $Ifs,
            efs_alloc: &'static mut littlefs2::fs::Allocation<$Efs>,
            efs_storage: &'static mut $Efs,
            vfs_alloc: &'static mut littlefs2::fs::Allocation<$Vfs>,
            vfs_storage: &'static mut $Vfs,
            // TODO: flag per backend?
            format: bool,
        ) -> littlefs2::io::Result<()> {

            use core::{
                mem::MaybeUninit,
            };
            use littlefs2::fs::{
                Allocation,
                Filesystem,
            };

            static mut IFS_ALLOC: MaybeUninit<&'static mut Allocation<$Ifs>> = MaybeUninit::uninit();
            static mut IFS_STORAGE: MaybeUninit<&'static mut $Ifs> = MaybeUninit::uninit();
            static mut IFS: Option<Filesystem<'static, $Ifs>> = None;

            static mut EFS_ALLOC: MaybeUninit<&'static mut Allocation<$Efs>> = MaybeUninit::uninit();
            static mut EFS_STORAGE: MaybeUninit<&'static mut $Efs> = MaybeUninit::uninit();
            static mut EFS: Option<Filesystem<'static, $Efs>> = None;

            static mut VFS_ALLOC: MaybeUninit<&'static mut Allocation<$Vfs>> = MaybeUninit::uninit();
            static mut VFS_STORAGE: MaybeUninit<&'static mut $Vfs> = MaybeUninit::uninit();
            static mut VFS: Option<Filesystem<'static, $Vfs>> = None;

            unsafe {
                // always need to format RAM
                Filesystem::format(vfs_storage).expect("can format");
                // this is currently a RAM fs too...
                Filesystem::format(efs_storage).expect("can format");

                if format {
                    Filesystem::format(ifs_storage).expect("can format");
                }

                IFS_ALLOC.as_mut_ptr().write(ifs_alloc);
                IFS_STORAGE.as_mut_ptr().write(ifs_storage);
                IFS = Some(Filesystem::mount(
                    &mut *IFS_ALLOC.as_mut_ptr(),
                    &mut *IFS_STORAGE.as_mut_ptr(),
                )?);
                let ifs = $crate::store::Fs::new(IFS.as_ref().unwrap());
                Self::ifs_ptr().write(ifs);

                EFS_ALLOC.as_mut_ptr().write(efs_alloc);
                EFS_STORAGE.as_mut_ptr().write(efs_storage);
                EFS = Some(Filesystem::mount(
                    &mut *EFS_ALLOC.as_mut_ptr(),
                    &mut *EFS_STORAGE.as_mut_ptr(),
                )?);
                let efs = $crate::store::Fs::new(EFS.as_ref().unwrap());
                Self::efs_ptr().write(efs);

                VFS_ALLOC.as_mut_ptr().write(vfs_alloc);
                VFS_STORAGE.as_mut_ptr().write(vfs_storage);
                VFS = Some(Filesystem::mount(
                    &mut *VFS_ALLOC.as_mut_ptr(),
                    &mut *VFS_STORAGE.as_mut_ptr(),
                )?);
                let vfs = $crate::store::Fs::new(VFS.as_ref().unwrap());
                Self::vfs_ptr().write(vfs);

                Ok(())

            }
        }
    }
}}

// TODO: replace this with "fs.create_dir_all(path.parent())"
pub fn create_directories<'s, S: LfsStorage>(
    fs: &Filesystem<'s, S>,
    path: &Path,
) -> Result<(), Error>
{
    // hprintln!("preparing {:?}", core::str::from_utf8(path).unwrap()).ok();
    let path_bytes = path.as_ref().as_bytes();

    for i in 0..path_bytes.len() {
        if path_bytes[i] == b'/' {
            let dir_bytes = &path_bytes[..i];
            let dir = PathBuf::from(dir_bytes);
            // let dir_str = core::str::from_utf8(dir).unwrap();
            // hprintln!("create dir {:?}", dir_str).ok();
            // fs.create_dir(dir).map_err(|_| Error::FilesystemWriteFailure)?;
            match fs.create_dir(&dir) {
                Err(littlefs2::io::Error::EntryAlreadyExisted) => {}
                Ok(()) => {}
                error => { panic!("{:?}", &error); }
            }
        }
    }
    Ok(())
}

pub type KeyMaterial = ByteBuf<MAX_SERIALIZED_KEY_LENGTH>;

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
pub struct SerializedKey {
   // r#type: KeyType,
   pub kind: KeyKind,
   pub value: ByteBuf<MAX_SERIALIZED_KEY_LENGTH>,
}

impl<'a> TryFrom<(KeyKind, &'a [u8])> for SerializedKey {
    type Error = Error;
    fn try_from(from: (KeyKind, &'a [u8])) -> Result<Self, Error> {
        Ok(SerializedKey {
            kind: from.0,
            value: ByteBuf::from_slice(from.1).map_err(|_| Error::InternalError)?,
        })
    }
}

/// Reads contents from path in location of store.
pub fn read<N: heapless::ArrayLength<u8>>(store: impl Store, location: StorageLocation, path: &Path) -> Result<ByteBuf<N>, Error> {
    match location {
        StorageLocation::Internal => store.ifs().read(path),
        StorageLocation::External => store.efs().read(path),
        StorageLocation::Volatile => store.vfs().read(path),
    }.map(|vec| ByteBuf::from(vec)).map_err(|_| Error::FilesystemReadFailure)
}

/// Writes contents to path in location of store.
pub fn write(store: impl Store, location: StorageLocation, path: &Path, contents: &[u8]) -> Result<(), Error> {
    match location {
        StorageLocation::Internal => store.ifs().write(path, contents),
        StorageLocation::External => store.efs().write(path, contents),
        StorageLocation::Volatile => store.vfs().write(path, contents),
    }.map_err(|_| Error::FilesystemWriteFailure)
}

/// Creates parent directory if necessary, then writes.
pub fn store(store: impl Store, location: StorageLocation, path: &Path, contents: &[u8]) -> Result<(), Error> {
    match location {
        StorageLocation::Internal => create_directories(store.ifs(), path)?,
        StorageLocation::External => create_directories(store.efs(), path)?,
        StorageLocation::Volatile => create_directories(store.vfs(), path)?,
    }
    write(store, location, path, contents)
}

pub fn delete(store: impl Store, location: StorageLocation, path: &Path) -> bool {
    let outcome = match location {
        StorageLocation::Internal => store.ifs().remove(path),
        StorageLocation::External => store.efs().remove(path),
        StorageLocation::Volatile => store.vfs().remove(path),
    };

    outcome.is_ok()
}

// pub fn delete_volatile(store: impl Store, handle: &ObjectHandle) -> bool {
//     let key_types = [
//         KeyType::Secret,
//         KeyType::Public,
//     ];

//     let success = key_types.iter().any(|key_type| {
//         let path = self.key_path(*key_type, handle);
//         store::delete(store, StorageLocation::Volatile, &path)
//     });

//     success
// }

// pub fn delete_anywhere(store: impl Store, handle: &ObjectHandle) -> bool {
//     let key_types = [
//         KeyType::Secret,
//         KeyType::Public,
//     ];

//     let locations = [
//         StorageLocation::Internal,
//         StorageLocation::External,
//         StorageLocation::Volatile,
//     ];

//     let success = key_types.iter().any(|key_type| {
//         let path = self.key_path(*key_type, handle);
//         locations.iter().any(|location| {
//             store::delete(store, *location, &path)
//         })
//     });

//     success
// }

