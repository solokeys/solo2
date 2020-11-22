//! An exampl deferred logger, generated as `delog!(Delogger, 1024, 1024, StdoutFlusher)`.
//!
//! It is included here for documentation purposes only.
//!
//! Do ensure that the `example` feature is not active in production!

use crate::flushers::StdoutFlusher;

crate::delog!(Delogger, 1024, 1024, StdoutFlusher);
