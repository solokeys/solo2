//! Fallible, panic-free versions of all the `log` logging macros.
//!
//! These macros appear at the root of the library, not here.

/// Fallible version of `log!`.
#[macro_export(local_inner_macros)]
macro_rules! try_log {
    (target: $target:expr, $lvl:expr, $message:expr) => ({
        let lvl = $lvl;
        if lvl <= $crate::upstream::STATIC_MAX_LEVEL && lvl <= $crate::upstream::max_level() {
            // ensure that $message is a valid format string literal
            let _ = $crate::upstream::__log_format_args!($message);
            $crate::__private_api_try_log_lit(
                $message,
                lvl,
                &($target, $crate::upstream::__log_module_path!(), $crate::upstream::__log_file!(), $crate::upstream::__log_line!()),
            )
        } else {
            Ok(())
        }
    });
    (target: $target:expr, $lvl:expr, $($arg:tt)+) => ({
        let lvl = $lvl;
        if lvl <= $crate::upstream::STATIC_MAX_LEVEL && lvl <= $crate::upstream::max_level() {
            $crate::__private_api_try_log(
                log::__log_format_args!($($arg)+),
                lvl,
                &($target, $crate::upstream::__log_module_path!(), $crate::upstream::__log_file!(), $crate::upstream::__log_line!()),
            )
        } else {
            Ok(())
        }
    });
    ($lvl:expr, $($arg:tt)+) => (try_log!(target: $crate::upstream::__log_module_path!(), $lvl, $($arg)+))
}

/// Fallible version of `debug!`.
#[macro_export(local_inner_macros)]
macro_rules! try_debug {
    (target: $target:expr, $($arg:tt)+) => (
        try_log!(target: $target, $crate::upstream::Level::Debug, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::upstream::Level::Debug, $($arg)+)
    )
}

/// Fallible version of `error!`.
#[macro_export(local_inner_macros)]
macro_rules! try_error {
    (target: $target:expr, $($arg:tt)+) => (
        try_log!(target: $target, $crate::upstream::Level::Error, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::upstream::Level::Error, $($arg)+)
    )
}

/// Fallible version of `info!`.
#[macro_export(local_inner_macros)]
macro_rules! try_info {
    (target: $target:expr, $($arg:tt)+) => (
        try_log!(target: $target, $crate::upstream::Level::Info, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::upstream::Level::Info, $($arg)+)
    )
}

/// Fallible version of `trace!`.
#[macro_export(local_inner_macros)]
macro_rules! try_trace {
    (target: $target:expr, $($arg:tt)+) => (
        try_log!(target: $target, $crate::upstream::Level::Trace, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::upstream::Level::Trace, $($arg)+)
    )
}

/// Fallible version of `warn!`.
#[macro_export(local_inner_macros)]
macro_rules! try_warn {
    (target: $target:expr, $($arg:tt)+) => (
        try_log!(target: $target, $crate::upstream::Level::Warn, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::upstream::Level::Warn, $($arg)+)
    )
}

