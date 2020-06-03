use core::convert::Infallible;
use crate::hal::traits::wg::digital::v2::InputPin;
use crate::hal::traits::wg::timer::CountDown;
use crate::hal::{
    self,
    drivers::pins,
    typestates::pin,
};
use crate::hal::drivers::timer;
use crate::hal::peripherals::{
    ctimer,
};
use solo_bee_traits::buttons::{
    ButtonPress,
    ButtonEdge,
    Buttons,
    Button,
};
use crate::hal::typestates::{
    init_state,
};
use crate::hal::time::*;
pub type UserButtonPin = pins::Pio1_9;
pub type WakeupButtonPin = pins::Pio1_18;
pub type UserButton = hal::Pin<UserButtonPin, pin::state::Gpio<pin::gpio::direction::Input>>;
pub type WakeupButton = hal::Pin<WakeupButtonPin, pin::state::Gpio<pin::gpio::direction::Input>>;

pub type ThreeButtons = XpressoButtons<ctimer::Ctimer3<init_state::Enabled>>;

// type CtimerEnabled = ;
// impl<P1,P2,P3, > TouchSensor<P1, P2, P3, >
// where P1: PinId, P2: PinId, P3: PinId

pub struct XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{
    last_state: Buttons,
    user_button: UserButton,
    wakeup_button: WakeupButton,
    timer: timer::Timer<CTIMER>,
}

impl <CTIMER> XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{
    pub fn new (timer: timer::Timer<CTIMER>, user_button: UserButton, wakeup_button: WakeupButton) -> XpressoButtons<CTIMER> {
        let buts = Buttons{
            top: user_button.is_high().ok().unwrap(),
            mid: wakeup_button.is_high().ok().unwrap(),
            bot: wakeup_button.is_high().ok().unwrap()
        };
        Self {
            user_button: user_button,
            wakeup_button: wakeup_button,
            last_state: buts,
            timer: timer,
        }
    }
}

impl<CTIMER> ButtonPress for XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{

    // A minimal button implementation for Xpresso
    fn is_pressed(&self, but: Button) -> bool {
        match but {
            Button::ButtonAny => {
                self.user_button.is_low().ok().unwrap() ||
                self.wakeup_button.is_low().ok().unwrap()
            }
            Button::ButtonTop => {
                self.user_button.is_low().ok().unwrap()
            }
            _ => {
                self.wakeup_button.is_low().ok().unwrap()
            }
        }
    }

    fn is_released(&self, but: Button) -> bool {
        match but {
            Button::ButtonAny => {
                self.user_button.is_high().ok().unwrap() ||
                self.wakeup_button.is_high().ok().unwrap()
            }
            Button::ButtonTop => {
                self.user_button.is_high().ok().unwrap()
            }
            _ => {
                self.wakeup_button.is_high().ok().unwrap()
            }
        }
    }

    fn get_status(&self) -> Buttons {
        Buttons {
            top: self.user_button.is_high().ok().unwrap(),
            mid: self.wakeup_button.is_high().ok().unwrap(),
            bot: self.wakeup_button.is_high().ok().unwrap(),
        }
    }
}

impl<CTIMER> XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{
    fn get_status_debounced(&mut self) -> Buttons {
        // first, remove jitter
        let mut new_state = self.get_status();
        self.timer.start(1.ms());
        nb::block!(self.timer.wait()).ok();
        let new_state2 = self.get_status();

        if new_state.bot != new_state2.bot {
            new_state.bot = self.last_state.bot;
        }
        if new_state.top != new_state2.top {
            new_state.top = self.last_state.top;
        }
        if new_state.mid != new_state2.mid{
            new_state.mid = self.last_state.mid;
        }

        new_state
    }

    fn read_button(&mut self, edge_type: bool) -> Button {

        let new_state = self.get_status_debounced();

        let mid_edge = (self.last_state.mid ^ new_state.mid) && (self.last_state.mid ^ edge_type);
        let top_edge = (self.last_state.top ^ new_state.top) && (self.last_state.mid ^ edge_type);
        let bot_edge = (self.last_state.bot ^ new_state.bot) && (self.last_state.mid ^ edge_type);

        self.last_state = new_state;

        if top_edge && bot_edge {
            Button::ButtonSides
        }
        else if mid_edge {
            Button::ButtonMid
        } else if top_edge {
            Button::ButtonTop
        } else if bot_edge {
            Button::ButtonBot
        } else {
            Button::ButtonNone
        }
    }
}

impl<CTIMER> ButtonEdge for XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{
    /// Non-blockingly wait for the button to be pressed.
    /// This is edge sensitive, meaning it will not complete successfully more than once
    /// per actual button press.
    /// Use block!(...) macro to actually block.
    fn wait_for_press(&mut self, but: Button) -> nb::Result<Button, Infallible> {
        let read_but = self.read_button(true);
        if read_but == but {
            Ok(but)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// Same as for wait_for_press, but waits for the release of the button.
    fn wait_for_release(&mut self, but: Button) -> nb::Result<Button, Infallible> {
        let read_but = self.read_button(false);
        if read_but == but {
            Ok(but)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// See wait_for_press
    fn wait_for_any_press(&mut self, ) -> nb::Result<Button, Infallible> {
        let but = self.read_button(true);
        if but != Button::ButtonNone {
            Ok(but)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// See wait_for_release
    fn wait_for_any_release(&mut self, ) -> nb::Result<Button, Infallible> {
        let but = self.read_button(false);
        if but != Button::ButtonNone {
            Ok(but)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

// impl
