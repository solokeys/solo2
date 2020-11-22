// use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    // println!("cargo:rerun-if-changed=build.rs");

    // let target = env::var("TARGET")?;
    // let cortex_m_dsp = target.starts_with("thumbv7em") || target.starts_with("thumbv8m.main");
    // if cortex_m_dsp {
    //     println!("cargo:rustc-cfg=cortex_m_dsp");
    // }

    Ok(())
}
