#![cfg_attr(not(feature = "std"), no_std)]

pub use cfg_if::cfg_if;
pub mod hex;
pub use hex::*;

pub type Result = core::result::Result::<(), core::convert::Infallible>;

#[macro_export]
macro_rules! stub_log {($($tt:tt)*) => {{
    if false {
        // this is to mark all variables as used when logs are stubbed out.
        #[allow(unused_must_use)]
        ($($tt)*);
        core::result::Result::<(), core::convert::Infallible>::Ok(())
    } else {
        core::result::Result::<(), core::convert::Infallible>::Ok(())
    }
}}}

pub fn stub_dump_hex (_bin: &[u8], _len: usize) -> Result {
    Ok(())
}

#[cfg(feature = "cortex-m")]
extern crate funnel;
#[cfg(feature = "cortex-m")]
pub use crate::funnel::*;

#[cfg(feature = "cortex-m")]
pub mod print_functions {
    // For embedded use
    pub use super::{Result, funnel, hex, hex::*};
    #[allow(unused_imports)]
    use funnel::Logger;
    pub use ufmt;
    pub use ufmt::{uwriteln, uwrite};
    pub use cortex_m_semihosting::{hprintln, hprint, dbg};

    // macro_rules! __log { ($($tt:tt)*) => {{
    //     use $crate::print_functions::ufmt;
    //     $crate::print_functions::funnel::uwrite!($crate::print_functions::funnel::Logger::get().unwrap(), $($tt)*)
    // }}}

    #[macro_export]
    macro_rules! __logln { ($($tt:tt)*) => {{
        use $crate::print_functions::ufmt;
        $crate::print_functions::funnel::uwriteln!($crate::print_functions::funnel::Logger::get().unwrap(), $($tt)*)
    }}}

    #[macro_export]
    macro_rules! __blocking_log {
        ($($tt:tt)*) => {{
            $crate::print_functions::hprint!($($tt)*)
        }}
    }

    #[macro_export]
    macro_rules! __blocking_logln {
        ($($tt:tt)*) => {{
            $crate::print_functions::hprintln!($($tt)*)
        }}
    }

    #[macro_export]
    macro_rules! __blocking_dbg {
        ($($tt:tt)*) => {{
            $crate::print_functions::dbg!($($tt)*)
        }}
    }

    #[allow(unused)]
    pub fn dump_hex(bin: &[u8], len: usize) -> Result {
        crate::cfg_if! {
            if #[cfg(all(any(feature = "all", feature = "info"), not(feature = "none")))] {
                {
                    let mut logger = Logger::get().unwrap();
                    for i in 0 .. len {
                        uwrite!(logger, "{} ", bin[i].hex()).ok();
                    }
                    uwriteln!(logger,"");
                    Ok(())
                }
            } else {
                Ok(())
            }
        }
    }

    pub mod blocking {
        pub use super::{hprint,hprintln};
        use super::*;

        #[allow(unused)]
        pub fn dump_hex(bin: &[u8], len: usize) -> Result {

            crate::cfg_if! {
                if #[cfg(all(any(feature = "all", feature = "info"), not(feature = "none")))]
                {
                    {
                        for i in 0 .. len {
                            crate::print_functions::blocking::hprint!("{} ", bin[i].hex()).ok();
                        }
                        crate::print_functions::blocking::hprintln!("").ok();
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[cfg(feature = "std")]
mod print_functions {
    // For PC use
    pub use super::Result;

    #[macro_export]
    macro_rules! __log {
        ($($tt:tt)*) => {{
            std::print!($($tt)*);
            if true {
                Ok(())
            } else {
                Err(())
            }
        }}
    }

    #[macro_export]
    macro_rules! __logln {
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
    macro_rules! __blocking_logln {
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
    macro_rules! __blocking_log {
        ($($tt:tt)*) => {{
            std::print!($($tt)*);
            if true {
                Ok(())
            } else {
                Err(())
            }
        }}
    }

    #[macro_export]
    macro_rules! __blocking_dbg {
        ($($tt:tt)*) => {{
            dbg!($($tt)*);
            if true {
                Ok(())
            } else {
                Err(())
            }
        }}
    }

    #[allow(unused)]
    pub fn dump_hex(bin: &[u8], len: usize) -> Result {
        crate::cfg_if! {
            if #[cfg(all(any(feature = "all", feature = "info"), not(feature = "none")))]
            {
                use super::*;

                for i in 0 .. len {
                    std::print!("{} ",  bin[i].hex() );
                }
                std::println!("");
            }
        }
        Ok(())
    }

    pub mod blocking {
        pub use super::dump_hex;
    }
}

// provide a 'root' set of feature gates.  Changing these will affect logs in all crates.
crate::cfg_if! {
    if #[cfg(all(any(feature = "all", feature = "info"), not(feature = "none")))] {
        #[macro_export]
        macro_rules! __info { ($($tt:tt)*) => {{{  $crate::__logln!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_info { ($($tt:tt)*) => {{{  $crate::__blocking_logln!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_dbg_gated { ($($tt:tt)*) => {{{  $crate::__blocking_dbg!($($tt)*)  }}}}
    } else {
        #[macro_export]
        macro_rules! __info { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_info { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_dbg_gated { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
    }
}

crate::cfg_if! {
    if #[cfg(all(any(feature = "all", feature = "warn"), not(feature = "none")))] {
        #[macro_export]
        macro_rules! __warn { ($($tt:tt)*) => {{{  $crate::__logln!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_warn { ($($tt:tt)*) => {{{  $crate::__blocking_logln!($($tt)*)  }}}}
    } else {
        #[macro_export]
        macro_rules! __warn { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_warn { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
    }
}

crate::cfg_if! {
    if #[cfg(all(any(feature = "all", feature = "debug"), not(feature = "none")))] {
        #[macro_export]
        macro_rules! __debug { ($($tt:tt)*) => {{{  $crate::__logln!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_debug { ($($tt:tt)*) => {{{  $crate::__blocking_logln!($($tt)*)  }}}}
    } else {
        #[macro_export]
        macro_rules! __debug { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_debug { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
    }
}

crate::cfg_if! {
    if #[cfg(all(any(feature = "all", feature = "error"), not(feature = "none")))] {
        #[macro_export]
        macro_rules! __error { ($($tt:tt)*) => {{{  $crate::__logln!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_error { ($($tt:tt)*) => {{{  $crate::__blocking_logln!($($tt)*)  }}}}
    } else {
        #[macro_export]
        macro_rules! __error { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
        #[macro_export]
        macro_rules! __blocking_error { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
    }
}


crate::cfg_if! {
    if #[cfg(not(feature = "none"))] {
        #[macro_export]
        macro_rules! write { ($string:expr) => {{{  $crate::__blocking_log!($string)  }}}}
    } else {
        #[macro_export]
        macro_rules! write { ($($tt:tt)*) => {{{  $crate::stub_log!($($tt)*)  }}}}
    }
}

#[cfg(feature = "cortex-m")]
pub use print_functions::dump_hex;

pub use __info as info;
pub use __warn as warn;
pub use __debug as debug;
pub use __error as error;

// Need to do this or:
// error: macro-expanded `macro_export` macros from the current crate cannot be referred to by absolute paths
pub use __blocking_info as __blocking_info2;
pub use __blocking_warn as __blocking_warn2;
pub use __blocking_error as __blocking_error2;
pub use __blocking_debug as __blocking_debug2;
pub use __blocking_dbg_gated as __blocking_dbg_gated2;

pub mod blocking {
    #[cfg(feature = "cortex-m")]
    pub use super::print_functions::blocking::dump_hex;

    pub use super::__blocking_info2 as info;
    pub use super::__blocking_warn2 as warn;
    pub use super::__blocking_debug2 as debug;
    pub use super::__blocking_error2 as error;
    pub use super::__blocking_dbg_gated2 as dbg;
}

// bit of a hack to fix rust issue with handling a nested $($body:tt)*
// https://github.com/rust-lang/rust/issues/35853#issuecomment-415993963
#[macro_export]
macro_rules! with_dollar_sign {
    ($($body:tt)*) => {
        macro_rules! __with_dollar_sign { $($body)* }
        __with_dollar_sign!($);
    }
}


// Define a feature gate wrapper around logging calls for crates to use 'locally' without affecting logs in other crates.
#[macro_export]
macro_rules! add {
    ($module_name:ident) => {
        $crate::with_dollar_sign! {
            ($d:tt) => {

                $crate::cfg_if! {
                    if #[cfg(all(any(feature = "log-all", feature = "log-info"), not(feature = "log-none")))] {
                        #[macro_export]
                        macro_rules! __info_wrapper          { ($d($d args:expr),* $d(,)?)  => {{{  $crate::info!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_info_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::blocking::info!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_dbg_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::blocking::dbg!($d($d args),*)  }}}}
                    } else {
                        #[macro_export]
                        macro_rules! __info_wrapper          { ($d($d args:expr),* $d(,)?) => {{{  $crate::stub_log!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_info_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::stub_log!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_dbg_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::stub_log!($d($d args),*)  }}}}
                    }
                }

                $crate::cfg_if! {
                    if #[cfg(all(any(feature = "log-all", feature = "log-warn"), not(feature = "log-none")))] {
                        #[macro_export]
                        macro_rules! __warn_wrapper          { ($d($d args:expr),* $d(,)?) => {{{  $crate::warn!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_warn_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::blocking::warn!($d($d args),*)  }}}}
                    } else {
                        #[macro_export]
                        macro_rules! __warn_wrapper          { ($d($d args:expr),* $d(,)?) => {{{  $crate::stub_log!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_warn_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::stub_log!($d($d args),*)  }}}}
                    } }

                $crate::cfg_if! {
                    if #[cfg(all(any(feature = "log-all", feature = "log-debug"), not(feature = "log-none")))] {
                        #[macro_export]
                        macro_rules! __debug_wrapper          { ($d($d args:expr),* $d(,)?) => {{{  $crate::debug!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_debug_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::blocking::debug!($d($d args),*)  }}}}
                    } else {
                        #[macro_export]
                        macro_rules! __debug_wrapper          { ($d($d args:expr),* $d(,)?) => {{{  $crate::stub_log!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_debug_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::stub_log!($d($d args),*)  }}}}
                    }
                }

                $crate::cfg_if! {
                    if #[cfg(all(any(feature = "log-all", feature = "log-error"), not(feature = "log-none")))] {
                        #[macro_export]
                        macro_rules! __error_wrapper          { ($d($d args:expr),* $d(,)?) => {{{  $crate::error!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_error_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::blocking::error!($d($d args),*)  }}}}
                    } else {
                        #[macro_export]
                        macro_rules! __error_wrapper          { ($d($d args:expr),* $d(,)?) => {{{  $crate::stub_log!($d($d args),*)  }}}}
                        #[macro_export]
                        macro_rules! __blocking_error_wrapper { ($d($d args:expr),* $d(,)?) => {{{  $crate::stub_log!($d($d args),*)  }}}}
                    }
                }

                pub mod $module_name {
                    pub use __info_wrapper as info;
                    pub use __warn_wrapper as warn;
                    pub use __debug_wrapper as debug;
                    pub use __error_wrapper as error;
                    $crate::cfg_if! {
                        if #[cfg(all(any(feature = "log-all", feature = "log-info"), not(feature = "log-none")))] {
                            pub use $crate::dump_hex;
                        } else {
                            pub use $crate::stub_dump_hex as dump_hex;
                        }
                    }
                    pub mod blocking {
                        pub use __blocking_info_wrapper as info;
                        pub use __blocking_warn_wrapper as warn;
                        pub use __blocking_debug_wrapper as debug;
                        pub use __blocking_error_wrapper as error;

                        pub use __blocking_dbg_wrapper as dbg;

                        $crate::cfg_if! {
                            if #[cfg(all(any(feature = "log-all", feature = "log-info"), not(feature = "log-none")))] {
                                pub use $crate::blocking::dump_hex;
                            } else {
                                pub use $crate::stub_dump_hex as dump_hex;
                            }
                        }
                    }
                }

            }
        }
    };
}

#[cfg(test)]
add!(logger);


#[cfg(test)]
fn test_add(x:u32, y:u32) -> u64 {
    return x as u64 + y as u64;
}

#[test]
fn test_print () {

    info!("root hex:").ok();
    dump_hex(&[0xaa,0xbb,0xcc], 3).ok();

    info!("root hex blocking:").ok();
    blocking::dump_hex(&[0xaa,0xbb,0xcc], 3).ok();

    logger::info!("crate hex:").ok();
    logger::dump_hex(&[0xaa,0xbb,0xcc], 3).ok();

    logger::info!("crate hex blocking:").ok();
    logger::blocking::dump_hex(&[0xaa,0xbb,0xcc], 3).ok();

    assert!( info!("root {}", "info").is_ok() );
    assert!( blocking::info!("root {} blocking", "info").is_ok() );

    assert!( warn!("root {}", "warn").is_ok() );
    assert!( blocking::warn!("root {} blocking", "warn").is_ok() );

    assert!( error!("root {}", "error").is_ok() );
    assert!( blocking::error!("root {} blocking", "error").is_ok() );

    assert!( debug!("root {}", "debug").is_ok() );
    assert!( blocking::debug!("root {} blocking", "debug").is_ok() );

    assert!( logger::info!("crate {}", "info").is_ok() );
    assert!( logger::blocking::info!("crate {} blocking", "info").is_ok() );

    assert!( logger::warn!("crate {}", "warn").is_ok() );
    assert!( logger::blocking::warn!("crate {} blocking", "warn").is_ok() );

    assert!( logger::error!("crate {}", "error").is_ok() );
    assert!( logger::blocking::error!("crate {} blocking", "error").is_ok() );

    assert!( logger::debug!("crate {}", "debug").is_ok() );
    assert!( logger::blocking::debug!("crate {} blocking", "debug").is_ok() );


    assert!( blocking::info!("root test macro {} {:?} {:?} {:?}", "info", test_add(5432, 1111), 234, -55,).is_ok() );
    assert!( logger::blocking::info!("crate test macro {} {:?} {:?} {:?}", "info", test_add(5432, 1111), 234, -55,).is_ok() );

    let z = 1;
    assert!( logger::blocking::dbg!(z).is_ok() );
    assert!( blocking::dbg!(z).is_ok() );

}
