//! An example deferred logger, generated as `delog!(Delogger, 4096, StdoutFlusher)`.
//!
//! It is included here for documentation purposes only.
//!
//! Do ensure that the `example` feature is not active in production!

use crate::flushers::StdoutFlusher;

crate::delog!(Delogger, 4096, StdoutFlusher);

crate::local_delog!();
