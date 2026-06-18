//! NVMC-backed littlefs2 0.7 storage for the nRF52840 internal flash region.
//!
//! Region: 0x000A_4000..0x000F_4000 (320 KiB = 80 × 4 KiB pages). Sits
//! between the app (ends at 0xA4000) and the Nordic Open Bootloader
//! (starts at 0xF4000).
//! Reads bypass `Nvmc` via memory-mapped flash (no state mutation needed).
//! Writes/erases go through `Nvmc` (handles NVMC.READY waits + page semantics).

use embedded_storage::nor_flash::NorFlash;
use generic_array::typenum::{U1, U256};
use nrf52840_hal::nvmc::Nvmc;
use nrf52840_pac::NVMC;

use littlefs2::driver::Storage;
use littlefs2::io::{Error as LfsError, Result as LfsResult};

pub const FILESYSTEM_BASE: usize = 0x000A_4000;
pub const FILESYSTEM_LEN: usize = 320 * 1024;
pub const PAGE_SIZE: usize = 4096;
pub const FS_BLOCK_COUNT: usize = FILESYSTEM_LEN / PAGE_SIZE;

pub struct InternalFlashStorage {
    nvmc: Nvmc<NVMC>,
}

impl InternalFlashStorage {
    pub fn new(nvmc: NVMC) -> Self {
        let storage: &'static mut [u8] =
            unsafe { core::slice::from_raw_parts_mut(FILESYSTEM_BASE as *mut u8, FILESYSTEM_LEN) };
        Self {
            nvmc: Nvmc::new(nvmc, storage),
        }
    }
}

impl Storage for InternalFlashStorage {
    const READ_SIZE: usize = 4;
    const WRITE_SIZE: usize = 4;
    const BLOCK_SIZE: usize = PAGE_SIZE;
    const BLOCK_COUNT: usize = FS_BLOCK_COUNT;
    const BLOCK_CYCLES: isize = -1;
    type CACHE_SIZE = U256;
    type LOOKAHEAD_SIZE = U1;

    fn read(&mut self, off: usize, buf: &mut [u8]) -> LfsResult<usize> {
        // Memory-mapped read. Equivalent to `self.nvmc.read(...)` but skips
        // the NVMC.READY wait (reads don't need it). littlefs is supposed to
        // keep off+len within BLOCK_COUNT * BLOCK_SIZE, but we re-check
        // before constructing the raw slice — an out-of-bounds read here
        // would land outside the dedicated FS region.
        let end = off.checked_add(buf.len()).ok_or(LfsError::IO)?;
        if end > FILESYSTEM_LEN {
            return Err(LfsError::IO);
        }
        let src =
            unsafe { core::slice::from_raw_parts((FILESYSTEM_BASE + off) as *const u8, buf.len()) };
        buf.copy_from_slice(src);
        Ok(buf.len())
    }

    fn write(&mut self, off: usize, data: &[u8]) -> LfsResult<usize> {
        self.nvmc
            .write(off as u32, data)
            .map_err(|_| LfsError::IO)?;
        Ok(data.len())
    }

    fn erase(&mut self, off: usize, len: usize) -> LfsResult<usize> {
        self.nvmc
            .erase(off as u32, (off + len) as u32)
            .map_err(|_| LfsError::IO)?;
        Ok(len)
    }
}
