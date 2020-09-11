use core::convert::Infallible;

/// Trio of buttons.
///
/// Buttons A and B can't reliably be distinguished by user, as being top/bottom or left/right
/// depends on the orientation of the device.
///
/// The expected user gestures can be:
/// - press
/// - squeeze
/// - release
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Button {
    A,
    B,
    Middle,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct State {
    pub a: bool,
    pub b: bool,
    pub middle: bool,
}

/// Implement on triple of buttons.
///
/// Only `is_pressed` needs to actually be implemented.
pub trait Press {

    /// Is the specific button currently pressed
    fn is_pressed(&self, button: Button) -> bool;

    /// Is the specific button currently released
    fn is_released(&self, button: Button) -> bool {
        !self.is_pressed(button)
    }

    /// Are both A and B buttons pressed simultaneously
    fn is_squeezed(&self) -> bool {
        self.is_pressed(Button::A) && self.is_pressed(Button::B)
    }

    /// Return the current state (pressed / released) of the three buttons.
    fn state(&self) -> State {
        State {
            a: self.is_pressed(Button::A),
            b: self.is_pressed(Button::B),
            middle: self.is_pressed(Button::Middle),
        }
    }

    /// Wait for all the buttons to be inactivated.  Level sensitive.
    fn wait_for_all_release(&self) -> nb::Result<(), Infallible> {
        let state = self.state();
        if !(state.a || state.b || state.middle) {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// Wait for the input state to be active.  Level sensitive.
    fn wait_for_state(&self, state: State) -> nb::Result<(), Infallible> {
        if self.state() == state {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

pub trait Edge {
    /// Wait for the given button to be pressed.  Edge sensitive, meaning this returns Ok only once per button press.
    fn wait_for_new_press(&mut self, button: Button) -> nb::Result<(), Infallible>;

    /// Wait for the given button to be released.  Edge sensitive, meaning this returns Ok only once per button release.
    fn wait_for_new_release(&mut self, button: Button) -> nb::Result<(), Infallible>;

    /// Wait for "squeeze" event (both A + B buttons).  Edge sensitive, meaning this returns Ok only once per button squeeze.
    fn wait_for_new_squeeze(&mut self) -> nb::Result<(), Infallible>;

    /// Wait for any press event(s), and return the state.  Edge sensitive, meaning this returns Ok only once per button press.
    fn wait_for_any_new_press(&mut self) -> nb::Result<Button, Infallible>;

    /// Wait for any release event(s), and return the state.  Edge sensitive, meaning this returns Ok only once per button release.
    fn wait_for_any_new_release(&mut self) -> nb::Result<Button, Infallible>;
}
