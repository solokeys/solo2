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
        try_log!(target: $target, $crate::Level::Debug, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::Level::Debug, $($arg)+)
    )
}

/// Fallible version of `error!`.
#[macro_export(local_inner_macros)]
macro_rules! try_error {
    (target: $target:expr, $($arg:tt)+) => (
        try_log!(target: $target, $crate::Level::Error, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::Level::Error, $($arg)+)
    )
}

/// Fallible version of `info!`.
#[macro_export(local_inner_macros)]
macro_rules! try_info {
    (target: $target:expr, $($arg:tt)+) => (
        try_log!(target: $target, $crate::Level::Info, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::Level::Info, $($arg)+)
    )
}

/// Fallible version of `trace!`.
#[macro_export(local_inner_macros)]
macro_rules! try_trace {
    (target: $target:expr, $($arg:tt)+) => (
        try_log!(target: $target, $crate::Level::Trace, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::Level::Trace, $($arg)+)
    )
}

/// Fallible version of `warn!`.
#[macro_export(local_inner_macros)]
macro_rules! try_warn {
    (target: $target:expr, $($arg:tt)+) => (
        try_log!(target: $target, $crate::Level::Warn, $($arg)+)
    );
    ($($arg:tt)+) => (
        try_log!($crate::Level::Warn, $($arg)+)
    )
}

/// Immediate version of `log!`.
#[macro_export(local_inner_macros)]
macro_rules! log_now {
    ($lvl:expr, $($arg:tt)+) => (
        log!(target: "!", $lvl, $($arg)+)
    );
}

/// Immediate version of `debug!`.
#[macro_export(local_inner_macros)]
macro_rules! debug_now {
    ($($arg:tt)+) => (
        log!(target: "!", $crate::Level::Debug, $($arg)+)
    );
}

/// Immediate version of `error!`.
#[macro_export(local_inner_macros)]
macro_rules! error_now {
    ($($arg:tt)+) => (
        log!(target: "!", $crate::Level::Error, $($arg)+)
    );
}

/// Immediate version of `info!`.
#[macro_export(local_inner_macros)]
macro_rules! info_now {
    ($($arg:tt)+) => (
        log!(target: "!", $crate::Level::Info, $($arg)+)
    );
}

/// Immediate version of `trace!`.
#[macro_export(local_inner_macros)]
macro_rules! trace_now {
    ($($arg:tt)+) => (
        log!(target: "!", $crate::Level::Trace, $($arg)+)
    );
}

/// Immediate version of `warn!`.
#[macro_export(local_inner_macros)]
macro_rules! warn_now {
    ($($arg:tt)+) => (
        log!(target: "!", $crate::Level::Warn, $($arg)+)
    );
}

/// Fallible immediate version of `log!`.
#[macro_export(local_inner_macros)]
macro_rules! try_log_now {
    ($lvl:expr, $($arg:tt)+) => (
        try_log!(target: "!", $lvl, $($arg)+)
    );
}

/// Fallible immediate version of `debug!`.
#[macro_export(local_inner_macros)]
macro_rules! try_debug_now {
    ($($arg:tt)+) => (
        try_log!(target: "!", $crate::Level::Debug, $($arg)+)
    );
}

/// Fallible immediate version of `error!`.
#[macro_export(local_inner_macros)]
macro_rules! try_error_now {
    ($($arg:tt)+) => (
        try_log!(target: "!", $crate::Level::Error, $($arg)+)
    );
}

/// Fallible immediate version of `info!`.
#[macro_export(local_inner_macros)]
macro_rules! try_info_now {
    ($($arg:tt)+) => (
        try_log!(target: "!", $crate::Level::Info, $($arg)+)
    );
}

/// Fallible immediate version of `trace!`.
#[macro_export(local_inner_macros)]
macro_rules! try_trace_now {
    ($($arg:tt)+) => (
        try_log!(target: "!", $crate::Level::Trace, $($arg)+)
    );
}

/// Fallible immediate version of `warn!`.
#[macro_export(local_inner_macros)]
macro_rules! try_warn_now {
    ($($arg:tt)+) => (
        try_log!(target: "!", $crate::Level::Warn, $($arg)+)
    );
}

