// There is a syntax issue with "repetitions in binding patterns for nested macros",
// with a workaround: https://github.com/rust-lang/rust/issues/35853#issuecomment-443110660
//
// This is why we want to  have `($)` expressions in the following, can just imagine they're not there.
//
// Unfortunately, I couldn't get this to work, so instead we use the weird `with_dollar_sign!` instead.

#[macro_export]
#[doc(hidden)]
macro_rules! with_dollar_sign {
    ($($body:tt)*) => {
        macro_rules! __with_dollar_sign { $($body)* }
        __with_dollar_sign!($);
    }
}

/// Generate logging macros that can be gated by library.
///
/// Realize that these macros are generated **in the namespace of the consuming library**, the one
/// that actally later makes calls to `local_warn!` etc.
///
/// To see this in action, compile documentation using `cargo doc --features example`, or inspect
/// the `gate-tests/` subdirectory.
#[macro_export]
macro_rules! local_delog {
    () => {
        $crate::with_dollar_sign! {
            ($d:tt) => {

                /// Local version of `log!`.
                #[macro_export(local_inner_macros)]
                macro_rules! local_log {
                    (target: $target:expr, $lvl:expr, $message:expr) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-info"), not(feature = "log-none")))]
                        $crate::log!(target: $target, $lvl, $message));
                    (target: $target:expr, $lvl:expr, $d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-info"), not(feature = "log-none")))]
                        $crate::log!(target: $target, $lvl, $d($arg)+));
                    ($lvl:expr, $d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-info"), not(feature = "log-none")))]
                        $crate::log!($lvl, $d($arg)+));
                }

                /// Local version of `debug!`.
                #[macro_export(local_inner_macros)]
                macro_rules! local_debug {
                    (target: $target:expr, $d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-debug"), not(feature = "log-none")))]
                        $crate::debug!(target: $target, $d($arg)+));
                    ($d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-debug"), not(feature = "log-none")))]
                        $crate::debug!($d($arg)+));
                }

                /// Local version of `error!`.
                #[macro_export(local_inner_macros)]
                macro_rules! local_error {
                    (target: $target:expr, $d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-error"), not(feature = "log-none")))]
                        $crate::error!(target: $target, $d($arg)+));
                    ($d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-error"), not(feature = "log-none")))]
                        $crate::error!($d($arg)+));
                }

                /// Local version of `info!`.
                #[macro_export(local_inner_macros)]
                macro_rules! local_info {
                    (target: $target:expr, $d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-info"), not(feature = "log-none")))]
                        $crate::info!(target: $target, $d($arg)+));
                    ($d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-info"), not(feature = "log-none")))]
                        $crate::info!($d($arg)+));
                }

                /// Local version of `trace!`.
                #[macro_export(local_inner_macros)]
                macro_rules! local_trace {
                    (target: $target:expr, $d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-trace"), not(feature = "log-none")))]
                        $crate::trace!(target: $target, $d($arg)+));
                    ($d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-trace"), not(feature = "log-none")))]
                        $crate::trace!($d($arg)+));
                }

                /// Local version of `warn!`.
                #[macro_export(local_inner_macros)]
                macro_rules! local_warn {
                    (target: $target:expr, $d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-warn"), not(feature = "log-none")))]
                        $crate::warn!(target: $target, $d($arg)+));
                    ($d($arg:tt)+) => (
                        #[cfg(all(any(feature = "log-all", feature = "log-warn"), not(feature = "log-none")))]
                        $crate::warn!($d($arg)+));
                }
            }
        }
    }
}
