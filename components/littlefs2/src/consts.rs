#![allow(non_camel_case_types)]

/// Re-export of `typenum::consts`.
pub use generic_array::typenum::consts::*;

pub const FILENAME_MAX_PLUS_ONE: u32 = 255 + 1;
// pub type PATH_DEFAULT_MAX = generic_array::typenum::consts::U255;
// pub const PATH_MAX: u32 = 255;
pub const PATH_MAX: usize = 255;
pub const PATH_MAX_PLUS_ONE: usize = PATH_MAX + 1;
pub const FILEBYTES_MAX: u32 = crate::ll::LFS_FILE_MAX as _;
pub const ATTRBYTES_MAX: u32 = 1_022;
pub type ATTRBYTES_MAX_TYPE = U1022;
pub const LOOKAHEADWORDS_SIZE: u32 = 16;

