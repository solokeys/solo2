//! # Deferred logging, for instance for `printf()`-debugging (*a.k.a* tracing)
//!
//! This is an implementation of the `log::Log` trait, suitable for use
//! in both embedded and desktop environments.
//!
//! Compared to existing approaches such as `ufmt`, `cortex-m-funnel` and `defmt`,
//! we pursue different values and requirements, namely:
//!
//! - **compatibility with the standard `core::fmt` traits and the standard `log` library API**.
//!   This means that, while libraries may "upgrade" their logging capabilities by using `delog`
//!   as drop-in replacement for their logging calls (see below), any existing library that already
//!   uses `log` is compatible. This, for us, is a huge win as opposed to using up "weirdness
//!   budget" for something as trivial and throw-away as simple logging.
//! - it follows that one can easily drop a `trace!("{:?}", &suspicious_object)` call at any time for
//!   any object that has a (possibly automatically derived) `Debug` trait implementation – without
//!   passing around structures and keeping on top of lifetimes.
//! - deferred logging: This is a typical "shared memory" logger, calls to `info!` etc.
//!   are not directly sent to their final output, but instead are stored in a circular buffer
//!   that is "drained" by calling `flush` on the logger at a convenient moment, for instance
//!   in an idle loop.
//! - immediate mode logging: Sometimes one wants to bypass the deferred flushing of logs,
//!   this is possible using either the little-known `target` argument to `info!` and friends
//!   with "!" as parameter, or using the additional `immediate_info!` and friends macros.
//! - ability to set log levels *per library, at compile-time*. This can be easily retro-fitted
//!   on existing `log`-based libraries, by adding the requisite features in `Cargo.toml` and
//!   replacing `log` with `delog`.
//! - the outcome is that one can leave useful logging calls in the library code, only to activate
//!   them in targeted ways, exactly as needed.
//! - helper macros to easily output binary byte arrays and slices in hexadecimal representations,
//!   which wrap the data in newtypes with custom `fmt::UpperHex` etc. implementations.
//!
//! **Non-goals**:
//!
//! - ultimate speed or code size: Our intention are "normal" logs, not the use of "logging" for streaming
//!   binary data to the host. While admittedly the `core::fmt`-ing facilities are not as efficient
//!   as one may hope, in our use cases we have sufficient flash and RAM to use these (and some
//!   hope that, someday, eventually, maybe, the formatting machinery will be revisited and
//!   improved at the root level, namely the language itself.)
//!
//! That said, we believe there is opportunity to extend `delog` in the `defmt` direction by
//! using, e.g., the `fmt::Binary` trait, newtypes and sentinel values to embed raw binary
//! represenations of data in abnormally time-critical situations without formatting, deferring
//! the extraction and actual formatting to some host-side mechanism.

#![cfg_attr(not(any(feature = "std", test)), no_std)]
use core::fmt;

pub use log as upstream;
pub use log::LevelFilter;
// TODO: figure out how to re-export `log` as module and `log!` as macro
// This way, at least we can re-export `log!`, but in a weird twist of fate,
// it also gets re-exported as `upstream!` (huh?!)
pub use log::{debug, error, info, log, log_enabled, trace, warn};

#[cfg(feature = "example")]
pub mod example;

#[cfg(any(feature="std", feature="semihosting", test))]
pub mod flushers;

pub mod hex;
mod logger;
pub use logger::{Delogger, TryLog, dequeue, enqueue, try_enqueue};
pub mod render;
mod try_log;

/// A way to pass on logs, user supplied
///
/// In embedded, this is intended to pend an interrupt
/// to send the logs off via (USB) serial, semihosting, or similar.
///
/// On PC, typical implemenation will just println! or eprintln!
pub trait Flusher: core::fmt::Debug + Send {
    fn flush(&self, logs: &str);
}

static mut LOGGER: Option<&'static dyn logger::TryLog> = None;

/// Returns a reference to the logger (as `TryLog` implementation)
pub fn trylogger() -> &'static mut Option<&'static dyn logger::TryLog> {
    unsafe { &mut LOGGER }
}

// WARNING: this is not part of the crate's public API and is subject to change at any time
#[doc(hidden)]
pub fn __private_api_try_log(
    args: fmt::Arguments,
    level: log::Level,
    &(target, module_path, file, line): &(&str, &'static str, &'static str, u32),
) -> core::result::Result<(), ()> {
    trylogger().ok_or(())?.try_log(
        &log::Record::builder()
            .args(args)
            .level(level)
            .target(target)
            .module_path_static(Some(module_path))
            .file_static(Some(file))
            .line(Some(line))
            .build(),
    )
}

// WARNING: this is not part of the crate's public API and is subject to change at any time
#[doc(hidden)]
pub fn __private_api_try_log_lit(
    message: &str,
    level: log::Level,
    &(target, module_path, file, line): &(&str, &'static str, &'static str, u32),
) -> core::result::Result<(), ()> {
    trylogger().ok_or(())?.try_log(
        &log::Record::builder()
            .args(format_args!("{}", message))
            .level(level)
            .target(target)
            .module_path_static(Some(module_path))
            .file_static(Some(file))
            .line(Some(line))
            .build(),
    )
}

