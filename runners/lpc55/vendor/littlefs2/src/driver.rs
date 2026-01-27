//! The `Storage`, `Read`, `Write` and `Seek` driver.
#![allow(non_camel_case_types)]

use generic_array::ArrayLength;
use littlefs2_sys as ll;

use crate::{
    io::Result,
};

/// Users of this library provide a "storage driver" by implementing this trait.
///
/// The `write` method is assumed to be synchronized to storage immediately.
/// littlefs provides more flexibility - if required, this could also be exposed.
/// Do note that due to caches, files still must be synched. And unfortunately,
/// this can't be automatically done in `drop`, since it needs mut refs to both
/// filesystem and storage.
///
/// The `*_SIZE` types must be `generic_array::typenume::consts` such as `U256`.
///
/// Why? Currently, associated constants can not be used (as constants...) to define
/// arrays. This "will be fixed" as part of const generics.
/// Once that's done, we can get rid of `generic-array`s, and replace the
/// `*_SIZE` types with `usize`s.
pub trait Storage {

    // /// Error type for user-provided read/write/erase methods
    // type Error = usize;

    /// Minimum size of block read in bytes. Not in superblock
    const READ_SIZE: usize;

    /// Minimum size of block write in bytes. Not in superblock
    const WRITE_SIZE: usize;

    /// Size of an erasable block in bytes, as unsigned typenum.
    /// Must be a multiple of both `READ_SIZE` and `WRITE_SIZE`.
    /// At least 128 (https://git.io/JeHp9). Stored in superblock.
    const BLOCK_SIZE: usize;

    /// Number of erasable blocks.
    /// Hence storage capacity is `BLOCK_COUNT * BLOCK_SIZE`
    const BLOCK_COUNT: usize;

    /// Suggested values are 100-1000, higher is more performant but
    /// less wear-leveled.  Default of -1 disables wear-leveling.
    /// Value zero is invalid, must be positive or -1.
    const BLOCK_CYCLES: isize = -1;

    /// littlefs uses a read cache, a write cache, and one cache per per file.
    /// Must be a multiple of `READ_SIZE` and `WRITE_SIZE`.
    /// Must be a factor of `BLOCK_SIZE`.
    type CACHE_SIZE: ArrayLength<u8>;

    /// littlefs itself has a `LOOKAHEAD_SIZE`, which must be a multiple of 8,
    /// as it stores data in a bitmap. It also asks for 4-byte aligned buffers.
    /// Hence, we further restrict `LOOKAHEAD_SIZE` to be a multiple of 32.
    /// Our LOOKAHEADWORDS_SIZE is this multiple.
    type LOOKAHEADWORDS_SIZE: ArrayLength<u32>;
    // type LOOKAHEAD_SIZE: ArrayLength<u8>;

    ///// Maximum length of a filename plus one. Stored in superblock.
    ///// Should default to 255+1, but associated type defaults don't exist currently.
    ///// At most 1_022+1.
    /////
    ///// TODO: We can't actually change this - need to pass on as compile flag
    ///// to the C backend.
    //type FILENAME_MAX_PLUS_ONE: ArrayLength<u8>;

    // /// Maximum length of a path plus one. Necessary to convert Rust string slices
    // /// to C strings, which requires an allocation for the terminating
    // /// zero-byte. If in doubt, set to `FILENAME_MAX_PLUS_ONE`.
    // /// Must be larger than `FILENAME_MAX_PLUS_ONE`.
    // type PATH_MAX_PLUS_ONE: ArrayLength<u8>;

    ///// Maximum size of file. Stored in superblock.
    ///// Defaults to 2_147_483_647 (or u31, to avoid sign issues in the C code).
    ///// At most 2_147_483_647.
    /////
    ///// TODO: We can't actually change this - need to pass on as compile flag
    ///// to the C backend.
    //const FILEBYTES_MAX: usize = ll::LFS_FILE_MAX as _;

    ///// Maximum size of custom attributes.
    ///// Should default to 1_022, but associated type defaults don't exists currently.
    ///// At most 1_022.
    /////
    ///// TODO: We can't actually change this - need to pass on as compile flag
    ///// to the C backend.
    //type ATTRBYTES_MAX: ArrayLength<u8>;

    /// Read data from the storage device.
    /// Guaranteed to be called only with bufs of length a multiple of READ_SIZE.
    fn read(&self, off: usize, buf: &mut [u8]) -> Result<usize>;
    /// Write data to the storage device.
    /// Guaranteed to be called only with bufs of length a multiple of WRITE_SIZE.
    fn write(&mut self, off: usize, data: &[u8]) -> Result<usize>;
    /// Erase data from the storage device.
    /// Guaranteed to be called only with bufs of length a multiple of BLOCK_SIZE.
    fn erase(&mut self, off: usize, len: usize) -> Result<usize>;
    // /// Synchronize writes to the storage device.
    // fn sync(&mut self) -> Result<usize>;
}

// in the future, try to split the megatrait `Storage` into pieces
// like this?
mod future {
    // content of "superblock"
    pub trait DiskFormat {
        // version, upper/lower half-word contain major/minor
        // const DISK_FORMAT_VERSION: u32,

        // block_size, block_count
        const BLOCK_SIZE: usize;
        const BLOCK_COUNT: usize;

        // name_max, file_max, attr_max
        type FILENAME_MAX_PLUS_ONE;
        const FILEBYTES_MAX: usize = super::ll::LFS_FILE_MAX as _;
        type ATTRBYTES_MAX;
    }

    pub trait Driver {
        const READ_SIZE: usize;
        const WRITE_SIZE: usize;

        const BLOCK_SIZE: usize;
        const BLOCK_COUNT: usize;

        // fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize>;
        // fn write(&mut self, offset: usize, data: &[u8]) -> Result<usize>;
        // fn erase(&mut self, offset: usize, len: usize) -> Result<usize>;
    }

    pub trait MemoryUsage {
        // TODO: this supposedly influences whether files are inlined or not. Clarify
        type CACHE_SIZE;
        type LOOKAHEADWORDS_SIZE;
    }

    pub trait RuntimeParameters {
        const BLOCK_CYCLES: isize = -1;
    }
}
