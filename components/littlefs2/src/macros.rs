// TODO: should add another backend that randomly returns less
// data than requested, to emphasize the difference between
// `io::Read::read` and `::read_exact`.
/// A configurable implementation of the Storage trait in memory.
#[macro_export]
macro_rules! ram_storage { (

    name=$Name:ident,
    backend=$Backend:ident,
    trait=$StorageTrait:path,
    erase_value=$erase_value:expr,
    read_size=$read_size:expr,
    write_size=$write_size:expr,
    cache_size_ty=$cache_size:path,
    block_size=$block_size:expr,
    block_count=$block_count:expr,
    lookaheadwords_size_ty=$lookaheadwords_size:path,
    filename_max_plus_one_ty=$filename_max_plus_one:path,
    path_max_plus_one_ty=$path_max_plus_one:path,
    result=$Result:ident,

) => {
        pub struct $Backend {
            buf: [u8; $block_size * $block_count],
        }

        impl Default for $Backend {
            fn default() -> Self {
                $Backend {
                    buf: [$erase_value; $block_size * $block_count],
                }
            }
        }

        pub struct $Name<'backend> {
            backend: &'backend mut $Backend,
        }

        impl<'backend> $Name<'backend> {
            const ERASE_VALUE: u8 = $erase_value;
            pub fn new(backend: &'backend mut $Backend) -> Self {
                $Name { backend }
            }
        }

        impl<'backend> $StorageTrait for $Name<'backend> {
            const READ_SIZE: usize = $read_size;
            const WRITE_SIZE: usize = $write_size;
            type CACHE_SIZE = $cache_size;
            const BLOCK_SIZE: usize = $block_size;
            const BLOCK_COUNT: usize = $block_count;
            type LOOKAHEADWORDS_SIZE = $lookaheadwords_size;

            fn read(&mut self, offset: usize, buf: &mut [u8]) -> $Result<usize> {
                let read_size: usize = Self::READ_SIZE;
                debug_assert!(offset % read_size == 0);
                debug_assert!(buf.len() % read_size == 0);
                for (from, to) in self.backend.buf[offset..].iter().zip(buf.iter_mut()) {
                    *to = *from;
                }
                Ok(buf.len())
            }

            fn write(&mut self, offset: usize, data: &[u8]) -> $Result<usize> {
                let write_size: usize = Self::WRITE_SIZE;
                debug_assert!(offset % write_size == 0);
                debug_assert!(data.len() % write_size == 0);
                for (from, to) in data.iter().zip(self.backend.buf[offset..].iter_mut()) {
                    *to = *from;
                }
                Ok(data.len())
            }

            fn erase(&mut self, offset: usize, len: usize) -> $Result<usize> {
                let block_size: usize = Self::BLOCK_SIZE;
                debug_assert!(offset % block_size == 0);
                debug_assert!(len % block_size == 0);
                for byte in self.backend.buf[offset..offset + len].iter_mut() {
                    *byte = Self::ERASE_VALUE;
                }
                Ok(len)
            }
        }
    };
    ($Name:ident, $Backend:ident, $bytes:expr) => {
        ram_storage!(
            name=$Name,
            backend=$Backend,
            trait=LfsStorage,
            erase_value=0xff,
            read_size=1,
            write_size=1,
            cache_size_ty=$crate::consts::U32,
            block_size=128,
            block_count=$bytes/128,
            lookaheadwords_size_ty=$crate::consts::U1,
            filename_max_plus_one_ty=$crate::consts::U256,
            path_max_plus_one_ty=$crate::consts::U256,
            result=LfsResult,
        );
    };
    (tiny) => {
        ram_storage!(
            name=RamStorage,
            backend=Ram,
            trait=driver::Storage,
            erase_value=0xff,
            read_size=32,
            write_size=32,
            cache_size_ty=$crate::consts::U32,
            block_size=128,
            block_count=8,
            lookaheadwords_size_ty=$crate::consts::U1,
            filename_max_plus_one_ty=$crate::consts::U256,
            path_max_plus_one_ty=$crate::consts::U256,
            result=Result,
        );
    };
    (large) => {
        ram_storage!(
            name=RamStorage,
            backend=Ram,
            trait=driver::Storage,
            erase_value=0xff,
            read_size=32,
            write_size=32,
            cache_size_ty=$crate::consts::U32,
            block_size=256,
            block_count=512,
            lookaheadwords_size_ty=$crate::consts::U4,
            filename_max_plus_one_ty=$crate::consts::U256,
            path_max_plus_one_ty=$crate::consts::U256,
            result=Result,
        );
    };
}

#[macro_export]
macro_rules! const_ram_storage { (

    name=$Name:ident,
    trait=$StorageTrait:path,
    erase_value=$erase_value:expr,
    read_size=$read_size:expr,
    write_size=$write_size:expr,
    cache_size_ty=$cache_size:path,
    block_size=$block_size:expr,
    block_count=$block_count:expr,
    lookaheadwords_size_ty=$lookaheadwords_size:path,
    filename_max_plus_one_ty=$filename_max_plus_one:path,
    path_max_plus_one_ty=$path_max_plus_one:path,
    result=$Result:ident,

) => {
        pub struct $Name {
            buf: [u8; $block_size * $block_count],
        }

        impl $Name {
            const ERASE_VALUE: u8 = $erase_value;
            pub const fn new() -> Self {
                // Self::default()
                Self { buf: [$erase_value; $block_size * $block_count] }
            }
        }

        impl Default for $Name {
            fn default() -> Self {
                Self {
                    buf: [$erase_value; $block_size * $block_count],
                }
            }
        }

        impl $StorageTrait for $Name {
            const READ_SIZE: usize = $read_size;
            const WRITE_SIZE: usize = $write_size;
            type CACHE_SIZE = $cache_size;
            const BLOCK_SIZE: usize = $block_size;
            const BLOCK_COUNT: usize = $block_count;
            type LOOKAHEADWORDS_SIZE = $lookaheadwords_size;

            fn read(&mut self, offset: usize, buf: &mut [u8]) -> $Result<usize> {
                let read_size: usize = Self::READ_SIZE;
                debug_assert!(offset % read_size == 0);
                debug_assert!(buf.len() % read_size == 0);
                for (from, to) in self.buf[offset..].iter().zip(buf.iter_mut()) {
                    *to = *from;
                }
                Ok(buf.len())
            }

            fn write(&mut self, offset: usize, data: &[u8]) -> $Result<usize> {
                let write_size: usize = Self::WRITE_SIZE;
                debug_assert!(offset % write_size == 0);
                debug_assert!(data.len() % write_size == 0);
                for (from, to) in data.iter().zip(self.buf[offset..].iter_mut()) {
                    *to = *from;
                }
                Ok(data.len())
            }

            fn erase(&mut self, offset: usize, len: usize) -> $Result<usize> {
                let block_size: usize = Self::BLOCK_SIZE;
                debug_assert!(offset % block_size == 0);
                debug_assert!(len % block_size == 0);
                for byte in self.buf[offset..offset + len].iter_mut() {
                    *byte = Self::ERASE_VALUE;
                }
                Ok(len)
            }
        }
    };
    ($Name:ident, $bytes:expr) => {
        const_ram_storage!(
            name=$Name,
            trait=LfsStorage,
            erase_value=0xff,
            read_size=16,
            write_size=512,
            cache_size_ty=$crate::consts::U512,
            block_size=512,
            block_count=$bytes/512,
            lookaheadwords_size_ty=$crate::consts::U1,
            filename_max_plus_one_ty=$crate::consts::U256,
            path_max_plus_one_ty=$crate::consts::U256,
            result=LfsResult,
        );
    };
}
