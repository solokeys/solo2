//! Runner-driven, non-blocking user-presence signal. The wallet requests UP and
//! polls the result; the runner computes the result cooperatively in its idle
//! loop (where the clock + button/field inputs live). No trussed round-trip, no
//! blocking — so the idle loop stays free for NFC during a wallet sign.
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Result codes.
pub const WAITING: u8 = 0;
pub const GRANTED: u8 = 1;
pub const TIMED_OUT: u8 = 2;

static UP_REQUEST: AtomicBool = AtomicBool::new(false);
static UP_RESULT: AtomicU8 = AtomicU8::new(WAITING);

/// Wallet: begin waiting for user presence.
pub fn request_up() {
    UP_RESULT.store(WAITING, Ordering::Release);
    UP_REQUEST.store(true, Ordering::Release);
}
/// Wallet: stop waiting (granted/denied/aborted).
pub fn clear_up() {
    UP_REQUEST.store(false, Ordering::Release);
    UP_RESULT.store(WAITING, Ordering::Release);
}
/// Runner: is a wallet sign currently waiting?
pub fn is_up_requested() -> bool {
    UP_REQUEST.load(Ordering::Acquire)
}
/// Runner: publish the computed result.
pub fn set_up_result(r: u8) {
    UP_RESULT.store(r, Ordering::Release);
}
/// Wallet: read the latest result.
pub fn up_result() -> u8 {
    UP_RESULT.load(Ordering::Acquire)
}
