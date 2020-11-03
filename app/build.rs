use std::{env, fs::File, io::Write, path::Path};
use std::process::Command;
use std::str;

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

    ($file:expr, $name:literal, $value:expr) => {
        writeln!($file, "pub const {}: &'static str = \"{}\";", $name, $value)
            .expect("Could not write build_constants.rs file");
    }
}

fn main(){
    let out_dir = env::var("OUT_DIR").expect("No out dir");
    let dest_path = Path::new(&out_dir).join("build_constants.rs");
    let mut f = File::create(&dest_path).expect("Could not create file");

    let hash_long_cmd = Command::new("git").args(&["rev-parse", "HEAD"]).output().unwrap().stdout;
    let hash_short_cmd = Command::new("git").args(&["rev-parse", "--short", "HEAD"]).output().unwrap().stdout;

    let hash_long =
        str::from_utf8(&hash_long_cmd).unwrap();
    let hash_short =
        str::from_utf8(&hash_short_cmd).unwrap();

    writeln!(&mut f, "pub mod build_constants {{").expect("Could not write build_constants.rs.");
    add_build_variable!(&mut f, "CARGO_PKG_VERSION_MAJOR", u8);
    add_build_variable!(&mut f, "CARGO_PKG_VERSION_MINOR", u8);
    add_build_variable!(&mut f, "CARGO_PKG_VERSION_PATCH", u8);

    add_build_variable!(&mut f, "CARGO_PKG_HASH", hash_long);
    add_build_variable!(&mut f, "CARGO_PKG_HASH_SHORT", hash_short);

    // Add integer version of the version number
    let version_bytes: [u8; 4] = [
        0u8,
        str::parse(env!("CARGO_PKG_VERSION_MAJOR")).unwrap(),
        str::parse(env!("CARGO_PKG_VERSION_MINOR")).unwrap(),
        str::parse(env!("CARGO_PKG_VERSION_PATCH")).unwrap(),
    ];
    let version:u32 = u32::from_be_bytes(version_bytes);

    add_build_variable!(&mut f, "CARGO_PKG_VERSION", version, u32);

    writeln!(&mut f, "}}").expect("Could not write build_constants.rs.");

}
