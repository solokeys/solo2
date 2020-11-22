//! Typical flushers in various environments.
//!
//! Availability based on cargo flags, e.g. `std` gives stdout/stderr flushers,
//! while `semihosting` gives flushers to host's stdout/stderr.

#[cfg(any(feature = "std", test))]
mod std;
#[cfg(any(feature = "std", test))]
pub use crate::flushers::std::*;

#[cfg(feature = "semihosting")]
mod semihosting;
#[cfg(feature = "semihosting")]
pub use crate::flushers::semihosting::*;
