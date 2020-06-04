// #[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! debug {
    ($($tt:tt)*) => {{ }}
}

// #[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! error {
    ($($tt:tt)*) => {{ }}
}

