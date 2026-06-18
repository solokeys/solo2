//! Board glue: trussed UserInterface + Syscall (shared); per-board pin
//! defs + HAL handles live in [`dk`] / [`solo`] (compiled in via the
//! `board-dk` / `board-solo` Cargo feature, mutually exclusive).
//!
//! Iteration 1 — minimum viable behaviour:
//!   * boot: LED4 blinks 3 × at startup (sync, in `LedController::new`).
//!   * idle: LED off.
//!   * waiting for user presence: LED on (solid).
//!   * any button (Btn1/Btn2/Btn3) approves; no deny / no cap-touch yet.

#[cfg(feature = "board-dk")]
pub mod dk;
#[cfg(feature = "board-dk")]
pub use dk::{init, Buttons, Leds};

#[cfg(feature = "board-solo")]
pub mod solo;
#[cfg(feature = "board-solo")]
pub use solo::{init, Buttons, Leds};

#[cfg(all(feature = "board-dk", feature = "board-solo"))]
compile_error!("`board-dk` and `board-solo` are mutually exclusive — pick one");

#[cfg(not(any(feature = "board-dk", feature = "board-solo")))]
compile_error!("enable one of `board-dk` / `board-solo` (default = `board-dk`)");

use core::time::Duration;
use cortex_m::peripheral::SCB;
use trussed::platform::{consent, reboot, ui};

#[derive(Default)]
pub struct Syscall;

impl trussed::client::Syscall for Syscall {
    #[inline]
    fn syscall(&mut self) {
        rtic::pend(nrf52840_pac::Interrupt::SWI0_EGU0);
    }
}

// ── Gesture detector (shared, board-agnostic) ────────────────────────────────
//
// Per-board `Buttons` exposes three primitive signals — left, right,
// explicit_deny. The detector commits a gesture on RELEASE so it can
// distinguish "single press → approve" from "both held → deny".
// Explicit-deny fires immediately on press, debounced one-shot.

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Gesture {
    None,
    Approve,
    Deny,
}

const DEBOUNCE_MS: u32 = 30;

pub struct GestureDetector {
    pending_started_ms: Option<u32>,
    both_seen: bool,
    last_explicit_deny: bool,
}

impl Default for GestureDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl GestureDetector {
    pub const fn new() -> Self {
        Self {
            pending_started_ms: None,
            both_seen: false,
            last_explicit_deny: false,
        }
    }

    pub fn poll(&mut self, left: bool, right: bool, explicit_deny: bool, now_ms: u32) -> Gesture {
        if explicit_deny && !self.last_explicit_deny {
            self.last_explicit_deny = true;
            self.pending_started_ms = None;
            self.both_seen = false;
            return Gesture::Deny;
        }
        if !explicit_deny {
            self.last_explicit_deny = false;
        }

        match (left, right) {
            (false, false) => {
                let result = match self.pending_started_ms {
                    Some(start) if now_ms.wrapping_sub(start) >= DEBOUNCE_MS => {
                        if self.both_seen {
                            Gesture::Deny
                        } else {
                            Gesture::Approve
                        }
                    }
                    _ => Gesture::None,
                };
                self.pending_started_ms = None;
                self.both_seen = false;
                result
            }
            (l, r) => {
                if self.pending_started_ms.is_none() {
                    self.pending_started_ms = Some(now_ms);
                }
                if l && r {
                    self.both_seen = true;
                }
                Gesture::None
            }
        }
    }
}

fn now_ms() -> u32 {
    use rtic_monotonics::Monotonic;
    crate::app::Mono::now().duration_since_epoch().to_millis()
}

// ── Test hook: probe-rs-driven user-presence override ───────────────────────
//
// Value semantics match `runners/pc/tests/support/up.rs`:
//   0   = no override (fall through to real button polling)
//   1   = approve once (Normal), consumed after one read
//   2   = approve once (Strong), consumed after one read
//   128 = deny sticky (every check returns None until reset to 0)
//   129 = approve sticky (every check returns Normal until reset to 0)
//
// Placed in `.uninit` with `no_mangle` so its address is stable and
// discoverable via the ELF symbol table. NEVER enable this feature on
// firmware shipped to users — it bypasses the real consent gate.
#[cfg(feature = "test-up-control")]
#[unsafe(link_section = ".uninit")]
#[unsafe(no_mangle)]
pub static mut UP_CONTROL: u8 = 0;

// ── Trussed UserInterface ────────────────────────────────────────────────────

pub struct UserInterface {
    leds: Leds,
    buttons: Buttons,
    gesture: GestureDetector,
    status: ui::Status,
}

impl UserInterface {
    /// Construct + run the boot indication: 3 quick LED blinks (sync).
    pub fn new(mut leds: Leds, buttons: Buttons) -> Self {
        const BLINK_CYCLES: u32 = 64_000_000 / 5 / 4; // ~200 ms at 64 MHz
        for _ in 0..3 {
            leds.set_brightness(255);
            cortex_m::asm::delay(BLINK_CYCLES);
            leds.set_brightness(0);
            cortex_m::asm::delay(BLINK_CYCLES);
        }
        Self {
            leds,
            buttons,
            gesture: GestureDetector::new(),
            status: ui::Status::Idle,
        }
    }
}

impl trussed::platform::UserInterface for UserInterface {
    fn check_user_presence(&mut self) -> consent::Level {
        // Probe-rs UP override: write `UP_CONTROL` from the host (via
        // probe-rs) to drive automated tests. One-shot values (1, 2) are
        // consumed; sticky (128, 129) persist until the host resets to 0.
        #[cfg(feature = "test-up-control")]
        {
            let val = unsafe { core::ptr::read_volatile(&raw const UP_CONTROL) };
            match val {
                0 => {
                    // No override — fall through.
                }
                1 => {
                    unsafe { core::ptr::write_volatile(&raw mut UP_CONTROL, 0) };
                    return consent::Level::Normal;
                }
                2 => {
                    unsafe { core::ptr::write_volatile(&raw mut UP_CONTROL, 0) };
                    return consent::Level::Strong;
                }
                128 => return consent::Level::None,
                129 => return consent::Level::Normal,
                // Strong sticky — required to drive CTAP 2.3 long-touch
                // Reset from a host that can't pulse the UP byte per
                // call (the only consent level that satisfies both
                // `user_present` and `user_present_strong`).
                130 => return consent::Level::Strong,
                _ => return consent::Level::None,
            }
        }

        // NFC: tap = consent (no button to press).
        if crate::nfct::field_on() {
            return consent::Level::Normal;
        }
        match self.gesture.poll(
            self.buttons.left(),
            self.buttons.right(),
            self.buttons.explicit_deny(),
            now_ms(),
        ) {
            Gesture::Approve => consent::Level::Normal,
            Gesture::Deny => {
                // ctaphid-dispatch sets the active app's InterruptFlag
                // to Working before calling us, so the CAS in interrupt()
                // succeeds and trussed's UP loop bails on the next iter.
                crate::types::interrupt_all_apps();
                consent::Level::None
            }
            Gesture::None => consent::Level::None,
        }
    }

    fn set_status(&mut self, status: ui::Status) {
        if self.status != status {
            self.status = status;
            let now_up = matches!(status, ui::Status::WaitingForUserPresence);
            self.leds.set_brightness(if now_up { 255 } else { 0 });
        }
    }

    fn status(&self) -> ui::Status {
        self.status
    }

    fn refresh(&mut self) {}

    fn uptime(&mut self) -> Duration {
        // Read RTIC SysTick monotonic — 64-bit ms count, doesn't wrap.
        // Was DWT cycle counter (32-bit, wraps at ~67 s). Trussed's UP loop
        // computes `nowtime - starttime` which panicked on wrap.
        use rtic_monotonics::Monotonic;
        let now = crate::app::Mono::now();
        Duration::from_millis(u64::from(now.duration_since_epoch().to_millis()))
    }

    fn reboot(&mut self, _to: reboot::To) -> ! {
        SCB::sys_reset()
    }

    fn wink(&mut self, _: Duration) {}
}
