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
        if val > 0 && val < 128 {
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

pub fn deny() {
    test_three_buttons().lock().unwrap().deny();
}

pub fn reset() {
    test_three_buttons().lock().unwrap().reset();
}
