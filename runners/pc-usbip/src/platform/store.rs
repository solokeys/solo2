// Imported from the https://github.com/trussed-dev/trussed-totp-pc-tutorial project
// Copyright (c) 2021 SoloKeys
// Licensed under either of Apache License, Version 2.0 or MIT license.

#![allow(missing_docs)]
//! Trussed stores are built on underlying `littlefs` implementations.
//!
//! Here, we use a single binary file-backed littlefs implementation for
//! persistent storage, and RAM array-backed implementations for the volatile storage.
use std::{fs::File, io::{Seek as _, SeekFrom}};

pub use generic_array::{GenericArray, typenum::{consts, U16, U128, U256, U512, U1022}};
use littlefs2::const_ram_storage;
use log::info;
use trussed::types::{LfsResult, LfsStorage};

const_ram_storage!(VolatileStorage, 1024*10);
// currently, `trussed` needs a dummy parameter here
const_ram_storage!(ExternalStorage, 1024*10);

trussed::store!(Store,
    Internal: FileFlash,
    External: ExternalStorage,
    Volatile: VolatileStorage
);

pub fn init_store(state_path: impl AsRef<std::path::Path>) -> Store {
    let filesystem = FileFlash::new(state_path);
    Store::attach_else_format(filesystem, ExternalStorage::new(), VolatileStorage::new())
}

pub struct FileFlash {
    path: std::path::PathBuf,
}

impl FileFlash {
    const SIZE: u64 = 128*1024;

    pub fn new(state_path: impl AsRef<std::path::Path>) -> Self {

        let path: std::path::PathBuf = state_path.as_ref().into();

        if let Ok(file) = File::open(&path) {
            assert_eq!(file.metadata().unwrap().len(), Self::SIZE);
        } else {
            // TODO: error handling
            let file = File::create(&path).unwrap();
            file.set_len(Self::SIZE).unwrap();
            info!("Created new state file");
        }
        Self { path }
    }
}

impl littlefs2::driver::Storage for FileFlash {
    const READ_SIZE: usize = 16;
    const WRITE_SIZE: usize = 16;
    const BLOCK_SIZE: usize = 512;

    const BLOCK_COUNT: usize = 128;
    const BLOCK_CYCLES: isize = -1;

    type CACHE_SIZE = U512;
    type LOOKAHEADWORDS_SIZE = U16;
    /// TODO: We can't actually be changed currently
    // type FILENAME_MAX_PLUS_ONE = U256;
    // type PATH_MAX_PLUS_ONE = U256;
    // const FILEBYTES_MAX: usize = littlefs2::ll::LFS_FILE_MAX as _;
    /// TODO: We can't actually be changed currently
    // type ATTRBYTES_MAX = U1022;


    fn read(&self, offset: usize, buffer: &mut [u8]) -> LfsResult<usize> {
        use std::io::Read;

        // debug!("reading {} bytes from {} in {:?}...", buffer.len(), offset, self.path);
        let mut file = File::open(&self.path).unwrap();
        file.seek(SeekFrom::Start(offset as _)).unwrap();
        let bytes_read = file.read(buffer).unwrap();
        assert_eq!(bytes_read, buffer.len());
        // debug!("..ok");
        Ok(bytes_read as _)
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> LfsResult<usize> {
        use std::io::Write;

        // debug!("writing {} bytes from {} in {:?}...", data.len(), offset, self.path);
        // debug!("{:?}", data);
        let mut file = std::fs::OpenOptions::new().write(true).open(&self.path).unwrap();
        file.seek(SeekFrom::Start(offset as _)).unwrap();
        let bytes_written = file.write(data).unwrap();
        assert_eq!(bytes_written, data.len());
        file.flush().unwrap();
        // debug!("..ok");
        Ok(bytes_written)
    }

    fn erase(&mut self, offset: usize, len: usize) -> LfsResult<usize> {
        use std::io::Write;

        // debug!("erasing {} bytes from {} in {:?}...", len, offset, self.path);
        let mut file = std::fs::OpenOptions::new().write(true).open(&self.path).unwrap();
        file.seek(SeekFrom::Start(offset as _)).unwrap();
        let zero_block = [0xFFu8; Self::BLOCK_SIZE];
        for _ in 0..(len/Self::BLOCK_SIZE) {
            let bytes_written = file.write(&zero_block).unwrap();
            assert_eq!(bytes_written, Self::BLOCK_SIZE);
        }
        file.flush().unwrap();
        // debug!("..ok");
        Ok(len)
    }

}

