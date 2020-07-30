#![cfg_attr(not(feature = "std"), no_std)]

pub mod hex;
pub use hex::*;


#[cfg(not(feature = "std"))]
mod print_functions {
    // For embedded use
    use super::*;
    use funnel::Logger;
    use ufmt::{uwriteln, uwrite};

    pub use funnel::{info, trace, warn, error, debug};

    pub fn dump_hex(bin: &[u8], len: usize){
        let mut logger = Logger::get().unwrap();
        if funnel::is_enabled(funnel::Level::Info) {
            for i in 0 .. len {
                uwrite!(logger, "{} ", bin[i].hex()).ok();
            }
            uwriteln!(logger,"").ok();
        }
    }
}

#[cfg(feature = "std")]
mod print_functions {
    // For PC use

    #[macro_export]
    macro_rules! info {
        ($($tt:tt)*) => {{
            std::println!($($tt)*);
            if true {
                Ok(())
            } else {
                Err(())
            }
        }}
    }

    #[macro_export]
    macro_rules! warn { ($($tt:tt)*) => {{
        info!($($tt)*)
    }}}
    #[macro_export]
    macro_rules! error { ($($tt:tt)*) => {{
        info!($($tt)*)
    }}}
    #[macro_export]
    macro_rules! trace { ($($tt:tt)*) => {{
        info!($($tt)*)
    }}}
    #[macro_export]
    macro_rules! debug { ($($tt:tt)*) => {{
        info!($($tt)*)
    }}}

    pub fn dump_hex(bin: &[u8], len: usize){
        use super::*;

        for i in 0 .. len {
            std::print!("{} ",  bin[i].hex() );
        }
        std::println!("");
    }
}

pub use print_functions::*;

#[test]
fn test_print () {
    assert!( info!("hello {}", 2).is_ok() );
}