use core::result::Result;
// use cortex_m_semihosting::{heprint,heprintln};
use crate::{
    drivers::{
        clocks::Clocks,
    },
    typestates::{
        init_state,
    }
};

#[derive(Copy,Clone,PartialEq)]
pub enum KeyType {
    Sbkek = 0x00,
    User = 0x01,
    Uds  = 0x02,
    PrinceRegion0 = 0x03,
    PrinceRegion1 = 0x04,
    PrinceRegion2 = 0x05,
}

#[derive(Copy,Clone)]
#[repr(C)]
pub struct IvCodePrinceRegion {
    pub keycode_header: u32,
    pub iv: [u8; 52],
}

#[derive(Copy,Clone)]
#[repr(C)]
pub struct Cfpa {
    pub header: u32,
    pub version: u32,
    pub secure_fw_version: u32,
    pub ns_fw_version: u32,
    pub image_key_revoke: u32,

    reserved0: [u8; 4],

    pub rotkh_revoke: u32,
    vendor_usage: u32,
    pub dcfg_ns_pin: u32,
    pub dcfg_ns_dflt: u32,
    enable_fa_mode: u32,

    reserved1: [u8; 4],
    // 12 * 4

    // + (4 + 52) * 3
    pub iv_code_prince_region: [IvCodePrinceRegion; 3],

    // + 40 + 224 + 32
    reserved2: [u8; 40],
    pub customer_data: [u8; 224],
    sha256: [u8; 32]
}

impl Cfpa {
    /// Check if everything has been done to set up a particular HW key.
    pub fn key_provisioned (&self, key_type: KeyType) -> bool {
        match key_type {
            // If there is a nonzero PRINCE IV in CFPA, then it must have provisioned.
            KeyType::PrinceRegion0 | KeyType::PrinceRegion1 | KeyType::PrinceRegion2 => {
                let mut iv_or = 0;
                let index = (key_type as usize) - (KeyType::PrinceRegion0 as usize);
                for i in 0 .. self.iv_code_prince_region[index].iv.len() {
                    iv_or |= self.iv_code_prince_region[index].iv[i];
                }

                iv_or != 0

            },
            // Not handling the other key types currently.
            _ => false
        }
    }
}

#[derive(Copy,Clone)]
#[repr(C)]
pub struct Cmpa {
    pub boot_cfg: u32,
    pub spi_flash_cfg: u32,
    pub usb_vid: u16,
    pub usb_pid: u16,
    pub sdio_cfg: u32,
    pub dcfg_pin: u32,
    pub dcfg_dflt: u32,
    pub dap_vendor_usage: u32,
    pub secure_boot_cfg: u32,
    pub prince_base_addr: u32,
    pub prince_sr: [u32; 3],
    reserved0: [u8; 32],

    pub rotkh: [u8; 32],
    reserved1: [u8; 144],

    pub customer_data: [u8; 224],
    sha256: [u8; 32]
}


// This compile time guarantees that Cmpa and Cfpa are 512 bytes.
#[allow(unreachable_code)]
fn _compile_time_assert() {
    unsafe {
        core::mem::transmute::<Cmpa, [u8; 512]>(panic!());
        core::mem::transmute::<Cfpa, [u8; 512]>(panic!());
    }
 }

// #define BOOTLOADER_API_TREE_POINTER (bootloader_tree_t*) 0x130010f0
#[repr(C)]
struct BootloaderTree {

    // All this does is a soft reset.
    run_bootloader: extern "C" fn(arg: &u32) -> (),

    version: u32,
    copyright: *const char,
    reserved0: u32,

    flash_driver: &'static FlashDriverInterface,

    // don't need these.
    reserved_kb_interface: u32,
    reserved1: [u32; 4],
    reserved_skboot_authenticate_interface: u32,
}

#[repr(C)]
struct FlashDriverInterface {
    version: u32,
    flash_init: unsafe extern "C" fn(config: &mut FlashConfig) -> u32,
    flash_erase: unsafe extern "C" fn(config: &mut FlashConfig, start: u32, length_in_bytes: u32, key: u32) -> u32,
    flash_program: unsafe extern "C" fn(config: &mut FlashConfig, start: u32, src: *const u8, length_in_bytes: u32) -> u32,
    flash_verify_erase: unsafe extern "C" fn(config: &mut FlashConfig, start: u32, length_in_bytes: u32) -> u32,
    flash_verify_program: unsafe extern "C" fn(config: &mut FlashConfig,
                                        start: u32, length_in_bytes: u32,
                                        expected_data: *const u8,
                                        failed_address: &mut u32,
                                        failed_data: &mut u32) -> u32,

    flash_get_property: unsafe extern "C" fn(config: &mut FlashConfig, tag: u32, value: &mut u32) -> u32,
    reserved: [u32; 3],

    ffr_init: unsafe extern "C" fn(config: &mut FlashConfig) -> u32,
    ffr_lock_all: unsafe extern "C" fn(config: &mut FlashConfig) -> u32,
    ffr_cust_factory_page_write: unsafe extern "C" fn(config: &mut FlashConfig, page_data: *const u8, seal_part: bool) -> u32,
    ffr_get_uuid: unsafe extern "C" fn(config: &mut FlashConfig, uuid: *mut u8) -> u32,
    ffr_get_customer_data: unsafe extern "C" fn(config: &mut FlashConfig, pData: *mut u8, offset: u32, len: u32) -> u32,

    // TODO
    ffr_keystore_write: unsafe extern "C" fn(config: &mut FlashConfig, ) -> u32,
    ffr_keystore_get_ac: unsafe extern "C" fn(config: &mut FlashConfig, activation_code: *mut u8) -> u32,
    ffr_keystore_get_kc: unsafe extern "C" fn(config: &mut FlashConfig, keycode: *mut u8, key_index: u32) -> u32,

    ffr_infield_page_write: unsafe extern "C" fn(config: &mut FlashConfig, page_data: *const u8, valid_len: u32) -> u32,
    ffr_get_customer_infield_data: unsafe extern "C" fn(config: &mut FlashConfig, page_data: *mut u8, offset: u32, len: u32) -> u32,
}

#[repr(C)]
pub struct FlashFfrConfig {
    pub ffr_block_base: u32,
    pub ffr_total_size: u32,
    pub ffr_page_size: u32,
    pub cfpa_page_version: u32,
    pub cfpa_page_offset: u32,
}

#[repr(C)]
pub struct FlashModeConfig {
    sys_freq_in_mhz: u32,
    single_word_mode: u32,
    write_mode: u32,
    read_mode: u32,
}

#[repr(C)]
pub struct FlashConfig {
    pflash_block_base: u32,
    pflash_total_size: u32,
    pflash_block_count: u32,
    pflash_page_size: u32,

    pflash_sector_size: u32,


    pub ffr_config: FlashFfrConfig,
    pub mode_config: FlashModeConfig,
}

impl FlashConfig {
    fn new(system_clock_freq_in_mhz: u32) -> FlashConfig {
        let flash_ffr_config = FlashFfrConfig {
            ffr_block_base: 0,
            ffr_total_size: 0,
            ffr_page_size: 0,
            cfpa_page_version: 0,
            cfpa_page_offset: 0,
        };
        let flash_mode_config = FlashModeConfig {
            sys_freq_in_mhz: system_clock_freq_in_mhz,
            single_word_mode: 0,
            write_mode: 0,
            read_mode: 0,
        };
        FlashConfig {
            pflash_block_base: 0,
            pflash_total_size: 0,
            pflash_block_count: 0,
            pflash_page_size: 0,
            pflash_sector_size: 0,
            ffr_config: flash_ffr_config,
            mode_config: flash_mode_config,
        }
    }
}

pub struct Pfr<State = init_state::Unknown> {
    pub flash_config: FlashConfig,
    pub _state: State,
}
impl<State> Pfr<State> {
    fn bootloader_api_tree() -> &'static mut BootloaderTree {
        unsafe { core::mem::transmute(0x130010f0u32 as *const ()) }
    }
    fn check_error(err: u32) -> Result<(), u32> {
        if err == 0 {
            Ok(())
        } else {
            Err(err)
        }
    }
}
impl Pfr{
    pub fn new() -> Self {
        Self {
            flash_config: FlashConfig::new(0),
            _state: init_state::Unknown,
        }
    }

    pub fn enabled(mut self, clock_config: &Clocks) -> Result<Pfr<init_state::Enabled>, u32> {

        self.flash_config = FlashConfig::new(clock_config.system_frequency.0/1000_000);

        let flash_init = Self::bootloader_api_tree().flash_driver.flash_init;
        let ffr_init = Self::bootloader_api_tree().flash_driver.ffr_init;

        Self::check_error( unsafe { flash_init(&mut self.flash_config) } )?;
        Self::check_error( unsafe { ffr_init(&mut self.flash_config) } )?;

        Ok(Pfr{
            flash_config: self.flash_config,
            _state: init_state::Enabled(())
        })
    }
}

impl Pfr <init_state::Enabled> {

    pub fn read_cmpa(&mut self) -> Result<Cmpa, u32> {
        let mut cmpa_bytes = [0u8; 512];

        let ffr_get_customer_data = Self::bootloader_api_tree().flash_driver.ffr_get_customer_data;

        Self::check_error(unsafe { ffr_get_customer_data(&mut self.flash_config, cmpa_bytes.as_mut_ptr(), 0, 512) })?;
        // heprintln!("cfpa:").ok();
        // dump_hex!(cfpa_bytes, 512);

        let cmpa: &Cmpa = unsafe{ core::mem::transmute(cmpa_bytes.as_ptr()) };

        Ok(*cmpa)
    }

    /// Get a readonly static reference to the customer data in CMPA.
    pub fn cmpa_customer_data(&mut self) -> &'static [u8] {
        let cmpa_ptr = (0x9E500) as *const u8;
        let slice = unsafe { core::slice::from_raw_parts(cmpa_ptr, 224) };
        slice
    }

    /// Keeping here for reference, but this sometimes returns unexpected old versions of the CFPA page that
    /// are not seen on scratch, ping, or pong pages.
    /// Findings:
    /// - Immediately after CFPA is updated, this method returns the latest CFPA data.
    /// - After boot/reset, this method will potentially return expected old versions of CFPA.
    /// - There is a pattern of how to increment VERSION to result in this method returning old CFPA versions or not which is impractical.
    /// It's almost like there is some other cfpa page storage not documented and this bootrom method mismanages the VERSION.
    pub fn read_cfpa_with_bootrom(&mut self) -> Result<Cfpa, u32> {
        let mut cfpa_bytes = [0u8; 512];

        let ffr_get_customer_infield_data = Self::bootloader_api_tree().flash_driver.ffr_get_customer_infield_data;

        Self::check_error( unsafe { ffr_get_customer_infield_data(&mut self.flash_config, cfpa_bytes.as_mut_ptr(), 0, 512) })?;
        // heprintln!("cfpa:").ok();
        // dump_hex!(cfpa_bytes, 512);

        let cfpa: &Cfpa = unsafe{ core::mem::transmute(cfpa_bytes.as_ptr()) };

        Ok(*cfpa)
    }

    /// Reads CFPA without use of bootrom.  Appears that the bootrom method sometimes
    /// returns previous versions of the CFPA page (not seen on scratch, ping, or pong pages).
    /// This method always returns the most recently updated Cfpa from ping or pong pages.
    pub fn read_latest_cfpa(&mut self) -> Result<Cfpa, u32> {
        let mut cfpa_bytes = [0u32; 128];

        let ping_ptr = (0x0009_DE00+512) as *const u32;
        let pong_ptr = (0x0009_DE00+512+512) as *const u32;

        let ping_version: u32 = unsafe {*(ping_ptr).offset(1)};
        let pong_version: u32 = unsafe {*(pong_ptr).offset(1)};

        let cfpa_ptr: *const u32 = if ping_version > pong_version {
            ping_ptr
        } else {
            pong_ptr
        };

        for i in 0..128 {
            cfpa_bytes[i] = unsafe { *cfpa_ptr.offset(i as isize) };
        }

        let cfpa: &Cfpa = unsafe{ core::mem::transmute(cfpa_bytes.as_ptr()) };

        Ok(*cfpa)
    }

    pub fn read_cfpa_ping(&mut self) -> Result<Cfpa, u32> {
        let mut cfpa_bytes = [0u32; 128];

        const CFPA_PTR: *const u32 = (0x0009_DE00+512) as *const u32;
        for i in 0..128 {
            cfpa_bytes[i] = unsafe { *CFPA_PTR.offset(i as isize) };
        }

        let cfpa: &Cfpa = unsafe{ core::mem::transmute(cfpa_bytes.as_ptr()) };

        Ok(*cfpa)
    }

    pub fn read_cfpa_pong(&mut self) -> Result<Cfpa, u32> {
        let mut cfpa_bytes = [0u32; 128];

        const CFPA_PTR: *const u32 = (0x0009_DE00+512+512) as *const u32;
        for i in 0..128 {
            cfpa_bytes[i] = unsafe { *CFPA_PTR.offset(i as isize) };
        }

        let cfpa: &Cfpa = unsafe{ core::mem::transmute(cfpa_bytes.as_ptr()) };

        Ok(*cfpa)
    }

    pub fn write_cfpa(&mut self, cfpa: &Cfpa) -> Result<(), u32> {

        let ffr_infield_page_write = Self::bootloader_api_tree().flash_driver.ffr_infield_page_write;
        let cfpa_bytes: *const u8 = unsafe{ core::mem::transmute( cfpa as *const Cfpa)};
        Self::check_error(
            unsafe{ ffr_infield_page_write (&mut self.flash_config, cfpa_bytes, 512) }
        )?;
        Ok(())
    }

    pub fn read_key_code(&mut self, key_type: KeyType) -> Result<[u8; 52], u32> {
        let mut bytes = [0u8; 52];
        let ffr_keystore_get_kc = Self::bootloader_api_tree().flash_driver.ffr_keystore_get_kc;

        Self::check_error(
            unsafe{ ffr_keystore_get_kc(&mut self.flash_config, bytes.as_mut_ptr(), key_type as u32) }
        )?;

        Ok(bytes)
    }

    pub fn read_activation_code(&mut self) -> Result<[u8; 1192], u32> {
        let mut ac = [0u8; 1192];
        let ffr_keystore_get_ac = Self::bootloader_api_tree().flash_driver.ffr_keystore_get_ac;
        Self::check_error(
            unsafe { ffr_keystore_get_ac(&mut self.flash_config, ac.as_mut_ptr()) }
        )?;

        Ok(ac)
    }

    /// Set write protection to PFR pages.  Lasts until next power on reset.
    pub fn lock_all(&mut self) -> Result<(), u32> {
        let ffr_lock_all= Self::bootloader_api_tree().flash_driver.ffr_lock_all;
        Self::check_error( unsafe { ffr_lock_all(&mut self.flash_config) } )?;
        Ok(())
    }

}
