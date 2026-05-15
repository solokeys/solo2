//! Host-side simulated hardware buttons for PC / Trussed tests.
//!
//! The API mirrors `runners/lpc55/board/src/traits/buttons.rs` (`Press` + `Edge`) so
//! `UserInterface::check_user_presence` can follow the same flow as the embedded
//! `board::trussed::UserInterface`. Automation uses the free functions
//! [`approve`], [`approve_sticky`], [`deny`], and [`reset`] (global singleton).

use core::convert::Infallible;

use nb;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

// --- Traits (aligned with `runners/lpc55/board/src/traits/buttons.rs`) ---

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Button {
    A,
    B,
    Middle,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub a: bool,
    pub b: bool,
    pub middle: bool,
}

pub trait Press {
    fn is_pressed(&self, button: Button) -> bool;
    fn is_released(&self, button: Button) -> bool {
        !self.is_pressed(button)
    }
    fn is_squeezed(&self) -> bool {
        self.is_pressed(Button::A) && self.is_pressed(Button::B)
    }
    fn state(&self) -> State {
        State {
            a: self.is_pressed(Button::A),
            b: self.is_pressed(Button::B),
            middle: self.is_pressed(Button::Middle),
        }
    }
    fn wait_for_all_release(&self) -> nb::Result<(), Infallible> {
        let state = self.state();
        if !(state.a || state.b || state.middle) {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
    fn wait_for_state(&self, state: State) -> nb::Result<(), Infallible> {
        if self.state() == state {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

pub trait Edge {
    fn wait_for_new_press(&mut self, button: Button) -> nb::Result<(), Infallible>;
    fn wait_for_new_release(&mut self, button: Button) -> nb::Result<(), Infallible>;
    fn wait_for_new_squeeze(&mut self) -> nb::Result<(), Infallible>;
    fn wait_for_any_new_press(&mut self) -> nb::Result<Button, Infallible>;
    fn wait_for_any_new_release(&mut self) -> nb::Result<Button, Infallible>;
}

// --- Shared atomic encoding for approve / sticky / deny / reset automation ---

const AUTO_APPROVE: u8 = 0;
const APPROVE_ONCE: u8 = 1;
const APPROVE_STICKY: u8 = 129;
const DENY_STICKY: u8 = 128;

static INSTANCE: OnceLock<Mutex<TestThreeButtons>> = OnceLock::new();

/// Global [`TestThreeButtons`] used by [`crate::UserInterface`] in the `test-buttons` build.
pub fn test_three_buttons() -> &'static Mutex<TestThreeButtons> {
    INSTANCE.get_or_init(|| Mutex::new(TestThreeButtons::new()))
}

pub struct TestThreeButtons {
    up_response: AtomicU8,
    held: Mutex<State>,
}

impl Default for TestThreeButtons {
    fn default() -> Self {
        Self::new()
    }
}

impl TestThreeButtons {
    pub fn new() -> Self {
        Self {
            up_response: AtomicU8::new(AUTO_APPROVE),
            held: Mutex::new(State {
                a: false,
                b: false,
                middle: false,
            }),
        }
    }

    /// Hold both A and B before the next [`Edge::wait_for_any_new_press`] so consent can map to
    /// [`trussed::platform::consent::Level::Strong`] (matches embedded `ThreeButtons` behaviour).
    pub fn set_held(&self, state: State) {
        *self.held.lock().unwrap() = state;
    }

    pub fn approve(&self) {
        self.up_response.store(APPROVE_ONCE, Ordering::SeqCst);
    }

    pub fn approve_sticky(&self) {
        self.up_response.store(APPROVE_STICKY, Ordering::SeqCst);
    }

    pub fn deny(&self) {
        self.up_response.store(DENY_STICKY, Ordering::SeqCst);
    }

    pub fn reset(&self) {
        self.up_response.store(AUTO_APPROVE, Ordering::SeqCst);
    }

    fn take_press_token(&self) -> bool {
        let val = self.up_response.load(Ordering::SeqCst);
        let grant = matches!(val, AUTO_APPROVE | APPROVE_ONCE | APPROVE_STICKY);
        if val == APPROVE_ONCE {
            let _ = self.up_response.compare_exchange(
                val,
                AUTO_APPROVE,
                Ordering::SeqCst,
                Ordering::Relaxed,
            );
        }
        grant
    }
}

impl Press for TestThreeButtons {
    fn is_pressed(&self, button: Button) -> bool {
        let h = self.held.lock().unwrap();
        match button {
            Button::A => h.a,
            Button::B => h.b,
            Button::Middle => h.middle,
        }
    }
}

impl Edge for TestThreeButtons {
    fn wait_for_new_press(&mut self, button: Button) -> nb::Result<(), Infallible> {
        if button != Button::A {
            return Err(nb::Error::WouldBlock);
        }
        if self.take_press_token() {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    fn wait_for_new_release(&mut self, _button: Button) -> nb::Result<(), Infallible> {
        Err(nb::Error::WouldBlock)
    }

    fn wait_for_new_squeeze(&mut self) -> nb::Result<(), Infallible> {
        Err(nb::Error::WouldBlock)
    }

    fn wait_for_any_new_press(&mut self) -> nb::Result<Button, Infallible> {
        if self.take_press_token() {
            Ok(Button::A)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    fn wait_for_any_new_release(&mut self) -> nb::Result<Button, Infallible> {
        Err(nb::Error::WouldBlock)
    }
}

pub fn approve() {
    test_three_buttons().lock().unwrap().approve();
}

pub fn approve_sticky() {
    test_three_buttons().lock().unwrap().approve_sticky();
}

// --- Single-shot UP-mode queue (vctaphid test driver) ---------------------
//
// Tests call `device.up_set_mode("tap" | "do_not_tap" | "long_tap")` which
// (on vctaphid) writes a magic packet. The daemon stores the requested
// mode here, *queued* — it doesn't take effect immediately because the
// surrounding test workflow can issue non-UP-needing requests between
// the mode-set and the request that actually needs UP. Instead, the
// `solo_pc::UserInterface::set_status(WaitingForUserPresence)` hook
// consumes the queued mode and arms the corresponding `buttons` state
// for the duration of *that one* UP wait, then resets back to
// approve_sticky so subsequent (non-UP-related) traffic isn't affected.

/// 0 = nothing queued (fall through to the default approve_sticky).
pub const UP_MODE_NONE: u8 = 0;
/// One tap: APPROVE_ONCE for the next UP poll loop.
pub const UP_MODE_TAP: u8 = 1;
/// Don't tap: DENY_STICKY for the next UP poll loop (trussed sees no
/// consent, times out, returns `UserActionTimeout`).
pub const UP_MODE_DO_NOT_TAP: u8 = 2;
/// Long-press: same `buttons` state as `UP_MODE_TAP` on vctaphid since
/// our held state is always (a:true, b:true), so any granted token
/// yields `Level::Strong`. Distinguished from `tap` only for the
/// human-facing prompt and for future runners that *can* tell short
/// from long presses apart.
pub const UP_MODE_LONG_TAP: u8 = 3;

static QUEUED_UP_MODE: AtomicU8 = AtomicU8::new(UP_MODE_NONE);

pub fn queue_up_mode(mode: u8) {
    QUEUED_UP_MODE.store(mode, Ordering::SeqCst);
}

/// Atomically take the queued mode (returning it) and reset the queue
/// back to `UP_MODE_NONE`. Called by the platform UI when trussed
/// signals it's entering a UP-poll loop.
pub fn take_queued_up_mode() -> u8 {
    QUEUED_UP_MODE.swap(UP_MODE_NONE, Ordering::SeqCst)
}

// --- "Don't grant before this instant" deadline -------------------------
//
// On real hardware UP costs the user a noticeable button press (~500 ms
// human reaction). usbd-ctaphid's keepalive timer relies on that —
// `did_start_processing` schedules the first KEEPALIVE 250 ms after a
// CBOR command starts, on the assumption that the authenticator is
// still inside its UP-poll loop by then. Our buttons-based UI grants
// instantly, so we never cross the 250 ms threshold and
// `test_keep_alive` sees no keepalives.
//
// To match the LPC55 wire behaviour we slow the *default* UP path
// (i.e. no test-queued mode) to artificially take ~300 ms. The
// `solo_pc::UserInterface::check_user_presence` hook reads this
// deadline before consulting the button state; if `Instant::now() <
// deadline`, it returns `None` (re-poll) regardless. Test-queued modes
// (`tap` / `do_not_tap` / `long_tap`) bypass the deadline because
// they're explicit "the human just acted" signals.

use std::time::Instant;

static UP_GRANT_NOT_BEFORE: Mutex<Option<Instant>> = Mutex::new(None);

pub fn set_up_grant_deadline(deadline: Option<Instant>) {
    *UP_GRANT_NOT_BEFORE.lock().unwrap() = deadline;
}

pub fn up_grant_deadline() -> Option<Instant> {
    *UP_GRANT_NOT_BEFORE.lock().unwrap()
}

pub fn deny() {
    test_three_buttons().lock().unwrap().deny();
}

pub fn reset() {
    test_three_buttons().lock().unwrap().reset();
}
