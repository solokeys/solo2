use std::{error, env, fs::File, io::Write, path::Path};
use std::process::Command;
use std::str;

/// Waiting on cargo fix!
// #[derive(serde::Deserialize)]
struct Config {
    parameters: Parameters,
}

// #[derive(serde::Deserialize)]
struct Parameters {
    filesystem_boundary: u32,
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

fn main() -> Result<(), Box<dyn error::Error>> {
    println!("cargo:rerun-if-changed=config/src/lib.rs");
    println!("cargo:rerun-if-changed=cfg.toml");

    let out_dir = env::var("OUT_DIR").expect("No out dir");

    // We would like to put configuration variables in cfg.toml, but due to a Cargo bug,
    // the serde feature flags will get merged with solo-bee's serde, causing build issues.
    // So this will remain here until the Cargo bug gets fixed.

    // let config = fs::read_to_string("cfg.toml")?;
    // let config: Config = toml::from_str(&config)?;

    // Hardcode until cargo issue gets fixed.
    let config = Config {parameters: Parameters{filesystem_boundary: 0x93_000}};


    let dest_path = Path::new(&out_dir).join("build_constants.rs");
    let mut f = File::create(&dest_path).expect("Could not create file");

    let hash_long_cmd = Command::new("git").args(&["rev-parse", "HEAD"]).output().unwrap().stdout;
    let hash_short_cmd = Command::new("git").args(&["rev-parse", "--short", "HEAD"]).output().unwrap().stdout;

    let hash_long =
        str::from_utf8(&hash_long_cmd[0..hash_long_cmd.len()-1]).unwrap();
    let hash_short =
        str::from_utf8(&hash_short_cmd[0..hash_short_cmd.len()-1]).unwrap();

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

    Ok(())
}
