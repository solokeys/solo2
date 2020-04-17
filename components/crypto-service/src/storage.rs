use core::convert::TryFrom;

#[cfg(feature = "semihosting")]
use cortex_m_semihosting::hprintln;
use serde_indexed::{DeserializeIndexed, SerializeIndexed};

use crate::config::*;
use crate::error::Error;
use crate::types::*;

// pub struct Efs<S: 'static + LfsStorage> {
//     fs: &'static mut S,
// }

// impl<S: 'static + LfsStorage> Efs<S> {
//     pub fn new(fs: &'static mut S) -> Self {
//         Self { fs }
//     }
// }

// pub struct Vfs<S: 'static + LfsStorage> {
//     fs: &'static mut S,
// }

// impl<S: 'static + LfsStorage> Vfs<S> {
//     pub fn new(fs: &'static mut S) -> Self {
//         Self { fs }
//     }
// }

//#[doc(hidden)]
//#[derive(Clone, Copy)]
//pub struct NotSendOrSync {
//    _inner: core::marker::PhantomData<*mut ()>,
//}

//#[doc(hidden)]
//impl NotSendOrSync {
//    /// Macro implementation detail
//    ///
//    /// # Safety
//    /// `unsafe` to prevent construction of singletons in safe code
//    pub unsafe fn new() -> Self {
//        Self {
//            _inner: core::marker::PhantomData,
//        }
//    }
//}

pub unsafe trait Store: Copy {
    type I: 'static + LfsStorage;
    // type E: 'static + LfsStorage;
    // type V: 'static + LfsStorage;
    fn ifs(self) -> &'static Fs<Self::I>;
    // fn efs(self) -> &'static Fs<Self::E>;
    // fn vfs(self) -> &'static Fs<Self::V>;
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
    $Ifs:ty
) => {
    #[derive(Clone, Copy)]
    pub struct $store {
        // __: $crate::storage::NotSendOrSync,
        __: core::marker::PhantomData<*mut ()>,
    }

    unsafe impl $crate::storage::Store for $store {
        type I = $Ifs;
        // type I = $Efs;
        // type I = $Ifs;

        fn ifs(self) -> &'static $crate::storage::Fs<$Ifs> {
            unsafe { &*Self::ifs_ptr() }
        }
    }

    impl $store {
        pub fn claim() -> Option<$store> {
            use core::sync::atomic::{AtomicBool, Ordering};
            // use $crate::storage::NotSendOrSync;

            static CLAIMED: AtomicBool = AtomicBool::new(false);

            if CLAIMED
                .compare_exchange_weak(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Some(Self { __: unsafe { $crate::storage::NotSendOrSync::new() } })
                Some(Self { __: core::marker::PhantomData })
            } else {
                None
            }
        }

        fn ifs_ptr() -> *mut $crate::storage::Fs<$Ifs> {
            use core::{cell::RefCell, mem::MaybeUninit};
            use $crate::storage::Fs;
            static mut IFS: MaybeUninit<Fs<$Ifs>> = MaybeUninit::uninit();
            unsafe { IFS.as_mut_ptr() }
        }

        pub fn mount(
            &self,
            ifs_alloc: &'static mut littlefs2::fs::Allocation<$Ifs>,
            ifs_storage: &'static mut $Ifs,
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
            // static mut IFS: MaybeUninit<Filesystem<'static, $Ifs>> = MaybeUninit::uninit();
            static mut IFS: Option<Filesystem<'static, $Ifs>> = None;

            unsafe {
                cortex_m_semihosting::hprintln!("trying to format").ok();
                if format {
                    Filesystem::format(ifs_storage).expect("can format");
                }
                cortex_m_semihosting::hprintln!("formatted").ok();

                IFS_ALLOC.as_mut_ptr().write(ifs_alloc);
                IFS_STORAGE.as_mut_ptr().write(ifs_storage);

                // let mut ifs = $crate::storage::Ifs::new(&mut littlefs2::fs::Filesystem::mount(
                //     &mut *IFS_ALLOC.as_mut_ptr(),
                //     &mut *IFS_STORAGE.as_mut_ptr(),
                // )?);

                IFS = Some(Filesystem::mount(
                    &mut *IFS_ALLOC.as_mut_ptr(),
                    &mut *IFS_STORAGE.as_mut_ptr(),
                )?);
                let mut ifs = $crate::storage::Fs::new(IFS.as_ref().unwrap());
                Self::ifs_ptr().write(ifs);

                Ok(())

            }
        }

    }

}}

fn store_key(store: impl Store, persistence: StorageLocation, path: &[u8], kind: KeyKind, key_bytes: &[u8]) -> Result<(), Error> {
    todo!();
}

pub struct TriStorage<'s, I: LfsStorage, E: LfsStorage, V: LfsStorage> {
    /// internal FLASH storage
    pub ifs: Filesystem<'s, I>,
    /// external FLASH storage
    pub efs: Filesystem<'s, E>,
    /// volatile RAM storage
    pub vfs: Filesystem<'s, V>,
}

// TODO: replace this with "fs.create_dir_all(path.parent())"
pub fn create_directories<'s, S: LfsStorage>(
    fs: &mut Filesystem<'s, S>,
    path: &[u8],
) -> Result<(), Error>
{
    // hprintln!("preparing {:?}", core::str::from_utf8(path).unwrap()).ok();
    for i in 0..path.len() {
        if path[i] == b'/' {
            let dir = &path[..i];
            // let dir_str = core::str::from_utf8(dir).unwrap();
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


pub fn store_serialized_key<'s, S: LfsStorage>(
    fs: &mut Filesystem<'s, S>,
    path: &[u8], buf: &[u8],
    user_attribute: Option<UserAttribute>,
)
    -> Result<(), Error>
{
    use littlefs2::fs::Attribute;

    // create directories if missing
    create_directories(fs, path)?;

    fs.write(path, buf).map_err(|_| Error::FilesystemWriteFailure)?;

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

pub(crate) fn delete<'s, S: LfsStorage>(fs: &mut Filesystem<'s, S>, path: &[u8]) -> bool {

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

        let (location, bytes): (_, Vec<u8, consts::U128>) =
            match self.vfs.read(path) {
                Ok(bytes) => (StorageLocation::Volatile, bytes),
                Err(_) => match self.ifs.read(path) {
                    Ok(bytes) => (StorageLocation::Internal, bytes),
                    Err(_) => match self.efs.read(path) {
                        Ok(bytes) => (StorageLocation::External, bytes),
                        Err(_) => return Err(Error::NoSuchKey),
                    }
                }
            };

        let serialized_key: SerializedKey =
            crate::cbor_deserialize(&bytes)
            .map_err(|_| Error::CborError)?;

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
    // OUTCOME: either ensure calling `.close()`, or patch the call in a `drop` for File.
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

