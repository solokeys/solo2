use crate::Flusher;

#[derive(Debug, Default)]
/// Flushes logs to host's stdout, via cortex-m-semihosting.
pub struct SemihostingFlusher {}

impl Flusher for SemihostingFlusher {
    fn flush(&self, logs: &str) {
        cortex_m_semihosting::hprint!("{}", logs).ok();
    }
}

#[derive(Debug, Default)]
/// Flushes logs to host's stderr, via cortex-m-semihosting.
pub struct SemihostingErrFlusher {}

impl Flusher for SemihostingErrFlusher {
    fn flush(&self, logs: &str) {
        cortex_m_semihosting::heprint!("{}", logs).ok();
    }
}
