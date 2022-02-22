use std::{error, env, fs::File, io::Write, path::Path};
use std::process::Command;
use std::str;

/// Waiting on cargo fix!
#[derive(serde::Deserialize)]
struct Config {
    parameters: Parameters,
}

#[derive(serde::Deserialize)]
struct Parameters {
    filesystem_boundary: u32,
}

#[derive(Eq, PartialEq)]
enum SocType {
    Lpc55,
    Nrf52840
}

macro_rules! add_build_variable{
    ($file:expr, $name:literal, u8) => {
        let value = env!($name);
        let value: u8 = str::parse(value).expect("Version components must be able to fit in a u8.");
        writeln!($file, "pub const {}: u8 = {};", $name, value)
            .expect("Could not write build_constants.rs file");
    };

    ($file:expr, $name:literal, $value:expr, u32) => {
        writeln!($file, "pub const {}: u32 = {};", $name, $value)
            .expect("Could not write build_constants.rs file");
    };

    ($file:expr, $name:literal, $value:expr, usize) => {
        writeln!($file, "pub const {}: usize = 0x{:x};", $name, $value)
            .expect("Could not write build_constants.rs file");
    };

    ($file:expr, $name:literal, $value:expr) => {
        writeln!($file, "pub const {}: &'static str = \"{}\";", $name, $value)
            .expect("Could not write build_constants.rs file");
    }
}

fn check_build_triplet() -> SocType {
    let target = env::var("TARGET").expect("$TARGET unset");
    let soc_is_lpc55 = env::var_os("CARGO_FEATURE_SOC_LPC55").is_some();
    let soc_is_nrf52840 = env::var_os("CARGO_FEATURE_SOC_NRF52840").is_some();

    if soc_is_lpc55 && !soc_is_nrf52840 {
        if target != "thumbv8m.main-none-eabi" {
            panic!("Wrong build triplet for LPC55, expecting thumbv8m.main-none-eabi, got {}", target);
        }
        SocType::Lpc55
    } else if soc_is_nrf52840 && !soc_is_lpc55 {
        if target != "thumbv7em-none-eabihf" {
            panic!("Wrong build triplet for NRF52840, expecting thumbv7em-none-eabihf, got {}", target);
        }
        SocType::Nrf52840
    } else {
        panic!("Multiple or no SOC features set.");
    }
}

fn generate_memory_x(outpath: &Path, template: &str, config: &Config) {
    let template = std::fs::read_to_string(template).expect("cannot read memory.x template file");
    let template = template.replace("##FLASH_LENGTH##", &format!("{}", config.parameters.filesystem_boundary >> 10));
    let template = template.replace("##FS_LENGTH##", &format!("{}", 630 - (config.parameters.filesystem_boundary >> 10)));
    let template = template.replace("##FS_BASE##", &format!("{:x}", config.parameters.filesystem_boundary));
    std::fs::write(outpath, template).expect("cannot write memory.x");
}

fn main() -> Result<(), Box<dyn error::Error>> {
    println!("cargo:rerun-if-changed=config/src/lib.rs");
    println!("cargo:rerun-if-changed=cfg.toml");

    let out_dir = env::var("OUT_DIR").expect("$OUT_DIR unset");
    let soc_type = check_build_triplet();

    let config = std::fs::read_to_string("cfg.toml").expect("cfg.toml not found");
    let config: Config = toml::from_str(&config).expect("cannot parse cfg.toml");
    if config.parameters.filesystem_boundary & 0x3ff != 0 {
        panic!("filesystem boundary is not a multiple of the flash block size (1KB)");
    }

    let dest_path = Path::new(&out_dir).join("build_constants.rs");
    let mut f = File::create(&dest_path).expect("Could not create file");

    let hash_long_cmd = Command::new("git").args(&["rev-parse", "HEAD"]).output().unwrap().stdout;
    let hash_short_cmd = Command::new("git").args(&["rev-parse", "--short", "HEAD"]).output().unwrap().stdout;

    let hash_long =
        str::from_utf8(&hash_long_cmd).unwrap().trim();
    let hash_short =
        str::from_utf8(&hash_short_cmd).unwrap().trim();

    writeln!(&mut f, "pub mod build_constants {{").expect("Could not write build_constants.rs.");
    add_build_variable!(&mut f, "CARGO_PKG_VERSION_MAJOR", u8);
    add_build_variable!(&mut f, "CARGO_PKG_VERSION_MINOR", u8);
    add_build_variable!(&mut f, "CARGO_PKG_VERSION_PATCH", u8);

    add_build_variable!(&mut f, "CARGO_PKG_HASH", hash_long);
    add_build_variable!(&mut f, "CARGO_PKG_HASH_SHORT", hash_short);

    // Add integer version of the version number
    let major: u32 = str::parse(env!("CARGO_PKG_VERSION_MAJOR")).unwrap();
    let minor: u32 = str::parse(env!("CARGO_PKG_VERSION_MINOR")).unwrap();
    let patch: u32 = str::parse(env!("CARGO_PKG_VERSION_PATCH")).unwrap();

    if major >= 1024 || minor > 9999 || patch >= 64 {
        panic!("config.firmware.product can at most be 1023.9999.63 for versions in customer data");
    }

    let version_to_check: u32 =
        (major << 22) |
        (minor << 6) | patch;

    add_build_variable!(&mut f, "CARGO_PKG_VERSION", version_to_check, u32);

    add_build_variable!(&mut f, "CONFIG_FILESYSTEM_BOUNDARY", config.parameters.filesystem_boundary, usize);

    writeln!(&mut f, "}}").expect("Could not write build_constants.rs.");

    if soc_type == SocType::Lpc55 {
        let memory_x = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("$CARGO_MANIFEST_DIR not set")).join("memory.x");
        generate_memory_x(&memory_x, "lpc55-memory-template.x", &config);
    }

    Ok(())
}
