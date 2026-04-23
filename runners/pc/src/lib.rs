//! Shared types for the PC runner, exposed as a library so tests can reuse them.

use littlefs2::fs::{Allocation, Filesystem};
use littlefs2::{const_ram_storage, consts};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{fs::File, io::Write};
use trussed::platform;
use trussed::platform::{consent, reboot, ui};
use trussed::store::DynFilesystem;

pub use generic_array::{
    typenum::{U1022, U16, U256, U512},
    GenericArray,
};

pub const SOLO_STATE: &str = "solo-state.bin";
pub const SIM_SOCKET_PATH: &str = "/tmp/solo2-sim.sock";
pub const SIM_UP_SOCKET_PATH: &str = "/tmp/solo2-sim-up.sock";

/// Incremented on every `UserInterface::check_user_presence` poll when `test-buttons` is enabled.
static USER_PRESENCE_POLLS: AtomicU64 = AtomicU64::new(0);

/// How many times Trussed has polled `check_user_presence` in this process (`test-buttons` only).
#[must_use]
pub fn user_presence_poll_count() -> u64 {
    USER_PRESENCE_POLLS.load(Ordering::Relaxed)
}

#[cfg(feature = "test-buttons")]
pub mod buttons;

#[allow(non_camel_case_types)]
pub mod littlefs_params {
    use super::*;
    pub const READ_SIZE: usize = 16;
    pub const WRITE_SIZE: usize = 512;
    pub const BLOCK_SIZE: usize = 512;

    pub const BLOCK_COUNT: usize = 256;
    pub const BLOCK_CYCLES: isize = -1;

    pub type CACHE_SIZE = U512;
    pub type LOOKAHEAD_SIZE = U16;
    pub type FILENAME_MAX_PLUS_ONE = U256;
    pub type PATH_MAX_PLUS_ONE = U256;
    pub const FILEBYTES_MAX: usize = littlefs2::ll::LFS_FILE_MAX as _;
    pub type ATTRBYTES_MAX = U1022;
}

pub struct FileFlash {
    state: [u8; 128 * 1024],
}
impl FileFlash {
    pub fn new() -> Self {
        let mut state = [0u8; 128 * 1024];
        if let Ok(contents) = std::fs::read(SOLO_STATE) {
            println!("loaded {}", SOLO_STATE);
            state.copy_from_slice(contents.as_slice());
            Self { state }
        } else {
            println!("No state yet, creating");
            Self { state }
        }
    }
}

impl Default for FileFlash {
    fn default() -> Self {
        Self::new()
    }
}

impl littlefs2::driver::Storage for FileFlash {
    const READ_SIZE: usize = littlefs_params::READ_SIZE;
    const WRITE_SIZE: usize = littlefs_params::WRITE_SIZE;
    const BLOCK_SIZE: usize = littlefs_params::BLOCK_SIZE;
    const BLOCK_COUNT: usize = littlefs_params::BLOCK_COUNT;
    const BLOCK_CYCLES: isize = littlefs_params::BLOCK_CYCLES;
    type CACHE_SIZE = littlefs_params::CACHE_SIZE;
    type LOOKAHEAD_SIZE = littlefs_params::LOOKAHEAD_SIZE;

    fn read(&mut self, off: usize, buf: &mut [u8]) -> littlefs2::io::Result<usize> {
        buf.copy_from_slice(&self.state[off..off + buf.len()]);
        Ok(buf.len())
    }

    fn write(&mut self, off: usize, data: &[u8]) -> littlefs2::io::Result<usize> {
        self.state[off..off + data.len()].copy_from_slice(data);
        let mut buffer = File::create(SOLO_STATE).unwrap();
        buffer.write_all(&self.state).unwrap();
        Ok(data.len())
    }

    fn erase(&mut self, off: usize, len: usize) -> littlefs2::io::Result<usize> {
        for byte in &mut self.state[off..off + len] {
            *byte = 0;
        }
        let mut buffer = File::create(SOLO_STATE).unwrap();
        buffer.write_all(&self.state).unwrap();
        Ok(len)
    }
}

const_ram_storage!(
    name = VolatileStorage,
    erase_value = 0x00,
    read_size = 1,
    write_size = 1,
    cache_size_ty = consts::U128,
    block_size = 128,
    block_count = 8192 / 128,
    lookahead_size_ty = consts::U8,
    filename_max_plus_one_ty = consts::U256,
    path_max_plus_one_ty = consts::U256,
);

const_ram_storage!(ExternalStorage, 1024);

#[derive(Clone, Copy)]
pub struct RunnerStore {
    ifs: &'static dyn DynFilesystem,
    efs: &'static dyn DynFilesystem,
    vfs: &'static dyn DynFilesystem,
}

impl trussed::store::Store for RunnerStore {
    fn ifs(&self) -> &dyn DynFilesystem {
        self.ifs
    }
    fn efs(&self) -> &dyn DynFilesystem {
        self.efs
    }
    fn vfs(&self) -> &dyn DynFilesystem {
        self.vfs
    }
}

pub type Store = RunnerStore;

/// Tracks simulated uptime for Trussed's user-consent timeout loop.
///
/// This must advance with real time: `UserInterface::uptime()` used to return a
/// fixed 1s value, which meant `(now - start) > timeout` was never true and a
/// denied user-presence check spun forever.
pub struct UserInterface {
    boot: std::time::Instant,
}

impl Default for UserInterface {
    fn default() -> Self {
        Self {
            boot: std::time::Instant::now(),
        }
    }
}

impl trussed::platform::UserInterface for UserInterface {
    fn check_user_presence(&mut self) -> consent::Level {
        #[cfg(feature = "test-buttons")]
        {
            use crate::buttons::{self, Edge, Press};
            USER_PRESENCE_POLLS.fetch_add(1, Ordering::Relaxed);
            let (state, press_result) = {
                let mut buttons = buttons::test_three_buttons().lock().unwrap();
                let state = buttons.state();
                let press_result = buttons.wait_for_any_new_press();
                (state, press_result)
            };
            if press_result.is_ok() {
                if state.a && state.b {
                    consent::Level::Strong
                } else {
                    consent::Level::Normal
                }
            } else {
                // Do not hold `test_three_buttons` mutex across sleep: the test thread may need
                // the lock for `up::approve()` / `reset()` on the next operation.
                std::thread::sleep(std::time::Duration::from_millis(50));
                consent::Level::None
            }
        }

        #[cfg(all(feature = "no-test-ui", not(feature = "test-buttons")))]
        {
            consent::Level::Normal
        }

        #[cfg(not(any(feature = "test-buttons", feature = "no-test-ui")))]
        {
            compile_error!(
                "solo-pc: enable the default `test-buttons` feature (simulated hardware UP for tests) \
                 or opt into `no-test-ui` for a non-interactive UI only (`--no-default-features --features no-test-ui`)."
            )
        }
    }

    fn set_status(&mut self, status: ui::Status) {
        println!("Set status: {:?}", status);
    }

    fn refresh(&mut self) {}

    fn uptime(&mut self) -> core::time::Duration {
        let elapsed = self.boot.elapsed();
        #[cfg(feature = "test-fast-up-clock")]
        {
            // fido-authenticator uses a 30 s UP window; Trussed busy-polls `check_user_presence`
            // until that **uptime delta** elapses. Scaling time shortens denied-UP wall time without
            // changing whether consent is granted (still driven by `buttons` / `up::`).
            const SCALE: u32 = 100;
            elapsed.saturating_mul(SCALE)
        }
        #[cfg(not(feature = "test-fast-up-clock"))]
        {
            elapsed
        }
    }

    fn reboot(&mut self, to: reboot::To) -> ! {
        println!("Restart!  ({:?})", to);
        std::process::exit(25);
    }
}

platform!(Board,
    R: chacha20::ChaCha8Rng,
    S: Store,
    UI: UserInterface,
);

/// Construct a mounted `RunnerStore` backed by the three heap-leaked filesystems.
pub fn mount_filesystems() -> RunnerStore {
    let internal_storage: &'static mut FileFlash = Box::leak(Box::new(FileFlash::new()));
    let internal_alloc: &'static mut Allocation<FileFlash> =
        Box::leak(Box::new(Filesystem::allocate()));

    let external_storage: &'static mut ExternalStorage =
        Box::leak(Box::new(ExternalStorage::new()));
    let external_alloc: &'static mut Allocation<ExternalStorage> =
        Box::leak(Box::new(Filesystem::allocate()));

    let volatile_storage: &'static mut VolatileStorage =
        Box::leak(Box::new(VolatileStorage::new()));
    let volatile_alloc: &'static mut Allocation<VolatileStorage> =
        Box::leak(Box::new(Filesystem::allocate()));

    if Filesystem::mount(internal_alloc, internal_storage).is_err() {
        println!("Not yet formatted!  Formatting..");
        Filesystem::format(internal_storage).unwrap();
    }
    let internal_fs: &'static mut Filesystem<'static, FileFlash> = Box::leak(Box::new(
        Filesystem::mount(internal_alloc, internal_storage).unwrap(),
    ));

    Filesystem::format(external_storage).unwrap();
    let external_fs: &'static mut Filesystem<'static, ExternalStorage> = Box::leak(Box::new(
        Filesystem::mount(external_alloc, external_storage).unwrap(),
    ));

    Filesystem::format(volatile_storage).unwrap();
    let volatile_fs: &'static mut Filesystem<'static, VolatileStorage> = Box::leak(Box::new(
        Filesystem::mount(volatile_alloc, volatile_storage).unwrap(),
    ));

    RunnerStore {
        ifs: internal_fs,
        efs: external_fs,
        vfs: volatile_fs,
    }
}
