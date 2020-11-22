//! The typical flushers in a `std` environment.

use crate::Flusher;

#[derive(Debug, Default)]
pub struct StdoutFlusher {}

impl Flusher for StdoutFlusher {
    fn flush(&self, logs: &str) {
        print!("{}", logs);
    }
}

#[derive(Debug, Default)]
pub struct StderrFlusher {}

impl Flusher for StderrFlusher {
    fn flush(&self, logs: &str) {
        eprint!("{}", logs);
    }
}

