#![cfg_attr(not(feature = "std"), no_std)]

pub mod hex;
pub use hex::*;


// I would put these in a mod "stub" but b.c. of how #[macro_export] moves macro to global root, it causes conflicts.
#[macro_export]
macro_rules! stub_info {($($tt:tt)*) => {{  if true { Ok(()) } else { Err(()) }  }}}
#[macro_export]
macro_rules! stub_warn {($($tt:tt)*) => {{  if true { Ok(()) } else { Err(()) }  }}}
#[macro_export]
macro_rules! stub_debug {($($tt:tt)*) => {{  if true { Ok(()) } else { Err(()) }  }}}
#[macro_export]
macro_rules! stub_error {($($tt:tt)*) => {{  if true { Ok(()) } else { Err(()) }  }}}
pub fn stub_dump_hex (_bin: &[u8], _len: usize){
}

#[cfg(not(feature = "std"))]
mod print_functions {
    // For embedded use
    use super::*;
    use funnel::Logger;
    use ufmt::{uwriteln, uwrite};

    pub use funnel::info as real_info;
    pub use funnel::warn as real_warn;
    pub use funnel::error as real_error;
    pub use funnel::debug as real_debug;

    pub fn dump_hex(bin: &[u8], len: usize){
        let mut logger = Logger::get().unwrap();
        if funnel::is_enabled(funnel::Level::Info) {
            for i in 0 .. len {
                uwrite!(logger, "{} ", bin[i].hex()).ok();
            }
            uwriteln!(logger,"").ok();
        }
    }

    pub mod blocking {
        use super::*;
        use cortex_m_semihosting::{hprintln, hprint};

        #[macro_export]
        macro_rules! blocking_info {
            ($($tt:tt)*) => {{
                hprintln!($($tt)*)
            }}
        }

        #[macro_export]
        macro_rules! blocking_warn { ($($tt:tt)*) => {{
            blocking_info!($($tt)*)
        }}}
        #[macro_export]
        macro_rules! blocking_error { ($($tt:tt)*) => {{
            blocking_info!($($tt)*)
        }}}
        #[macro_export]
        macro_rules! blocking_debug { ($($tt:tt)*) => {{
            blocking_info!($($tt)*)
        }}}

        pub fn dump_hex(bin: &[u8], len: usize){
            for i in 0 .. len {
                hprint!("{} ", bin[i].hex()).ok();
            }
            hprintln!("").ok();
        }


    }
}

#[cfg(feature = "std")]
mod print_functions {
    // For PC use

    #[macro_export]
    macro_rules! real_info {
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
    macro_rules! real_warn { ($($tt:tt)*) => {{
        real_info!($($tt)*)
    }}}
    #[macro_export]
    macro_rules! real_error { ($($tt:tt)*) => {{
        real_info!($($tt)*)
    }}}
    #[macro_export]
    macro_rules! real_debug { ($($tt:tt)*) => {{
        real_info!($($tt)*)
    }}}

    pub fn dump_hex(bin: &[u8], len: usize){
        use super::*;

        for i in 0 .. len {
            std::print!("{} ",  bin[i].hex() );
        }
        std::println!("");
    }

    #[macro_export]
    macro_rules! blocking_info { ($($tt:tt)*) => {{
        real_info!($($tt)*)
    }}}
    #[macro_export]
    macro_rules! blocking_warn { ($($tt:tt)*) => {{
        real_info!($($tt)*)
    }}}
    #[macro_export]
    macro_rules! blocking_error { ($($tt:tt)*) => {{
        real_info!($($tt)*)
    }}}
    #[macro_export]
    macro_rules! blocking_debug { ($($tt:tt)*) => {{
        real_info!($($tt)*)
    }}}

    pub mod blocking {
        pub use super::dump_hex;
    }
}

cfg_if::cfg_if! {
    if #[cfg(any(feature = "info", feature = "all"))] {
        pub use real_info as info;
        pub use print_functions::dump_hex;

    } else {
        pub use stub_info as info;
        pub use stub_dump_hex as dump_hex;
    }
}

cfg_if::cfg_if! {
    if #[cfg(any(feature = "warn", feature = "all"))] {
        pub use real_warn as warn;
    } else {
        pub use stub_warn as warn;
    }
}

cfg_if::cfg_if! {
    if #[cfg(any(feature = "debug", feature = "all"))] {
        pub use real_debug as debug;
    } else {
        pub use stub_debug as debug;
    }
}

cfg_if::cfg_if! {
    if #[cfg(any(feature = "error", feature = "all"))] {
        pub use real_error as error;
    } else {
        pub use stub_error as error;
    }
}


mod blocking {

    cfg_if::cfg_if! {
        if #[cfg(any(feature = "info", feature = "all"))] {
            pub use super::blocking_info as info;
            pub use super::print_functions::blocking::dump_hex;

        } else {
            pub use super::stub_info as info;
            pub use super::stub_dump_hex as dump_hex;
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(any(feature = "warn", feature = "all"))] {
            pub use super::blocking_warn as warn;
        } else {
            pub use stub_warn as warn;
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(any(feature = "debug", feature = "all"))] {
            pub use super::blocking_debug as debug;
        } else {
            pub use super::stub_debug as debug;
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(any(feature = "error", feature = "all"))] {
            pub use super::blocking_error as error;
        } else {
            pub use super::stub_error as error;
        }
    }
}



#[test]
fn test_print () {
    assert!( info!("log {}", "info").is_ok() );
    dump_hex(&[1,2,3], 2);

    assert!( blocking::info!("blocking log {}", "info").is_ok() );
    blocking::dump_hex(&[1,2,3], 2);

    assert!( warn!("log {}", "warn").is_ok() );
    assert!( blocking::warn!("blocking log {}", "warn").is_ok() );

    assert!( error!("log {}", "error").is_ok() );
    assert!( blocking::error!("blocking log {}", "error").is_ok() );

    assert!( debug!("log {}", "debug").is_ok() );
    assert!( blocking::debug!("blocking log {}", "debug").is_ok() );
}