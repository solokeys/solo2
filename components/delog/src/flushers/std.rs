use crate::Flusher;

#[derive(Debug, Default)]
/// Flushes logs to stdout.
pub struct StdoutFlusher {}

impl Flusher for StdoutFlusher {
    fn flush(&self, logs: &str) {
        print!("{}", logs);
    }
}

#[derive(Debug, Default)]
/// Flushes logs to stderr.
pub struct StderrFlusher {}

impl Flusher for StderrFlusher {
    fn flush(&self, logs: &str) {
        eprint!("{}", logs);
    }
}

