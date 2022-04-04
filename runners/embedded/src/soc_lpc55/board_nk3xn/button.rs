use core::convert::Infallible;
use lpc55_hal::{
	drivers::{pins, timer},
	peripherals::ctimer,
	time::DurationExtensions,
	traits::wg::{
		digital::v2::InputPin,
		timer::CountDown
	},
	typestates::{init_state, pin},
};
use crate::traits::buttons::{
	Button, Edge, Press, State
};

pub type UserButtonPin = pins::Pio0_31;
// pub type WakeupButtonPin = pins::Pio1_18;
pub type UserButton = lpc55_hal::Pin<UserButtonPin, pin::state::Gpio<pin::gpio::direction::Input>>;
// pub type WakeupButton = lpc55_hal::Pin<WakeupButtonPin, pin::state::Gpio<pin::gpio::direction::Input>>;

pub type ThreeButtons = XpressoButtons<ctimer::Ctimer1<init_state::Enabled>>;

// type CtimerEnabled = ;
// impl<P1,P2,P3, > TouchSensor<P1, P2, P3, >
// where P1: PinId, P2: PinId, P3: PinId

pub struct XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{
    last_state: State,
    user_button: UserButton,
    // wakeup_button: WakeupButton,
    timer: timer::Timer<CTIMER>,
}

impl <CTIMER> XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{
    // pub fn new (timer: timer::Timer<CTIMER>, user_button: UserButton, wakeup_button: WakeupButton) -> XpressoButtons<CTIMER> {
    pub fn new (timer: timer::Timer<CTIMER>, gpio: &mut lpc55_hal::Gpio<lpc55_hal::Enabled>, iocon: &mut lpc55_hal::Iocon<lpc55_hal::Enabled>) -> XpressoButtons<CTIMER> {
        let user_button = UserButtonPin::take().unwrap().into_gpio_pin(iocon, gpio).into_input();
        // let wakeup_button = WakeupButtonPin::take().unwrap().into_gpio_pin(iocon, gpio).into_input();
        let buts = State {
            a: user_button.is_high().ok().unwrap(),
            b: user_button.is_high().ok().unwrap(),
            middle: user_button.is_high().ok().unwrap(),
        };
        Self {
            user_button: user_button,
            // wakeup_button: wakeup_button,
            last_state: buts,
            timer: timer,
        }
    }
}

impl<CTIMER> Press for XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{

    // A minimal button implementation for Xpresso
    fn is_pressed(&mut self, but: Button) -> bool {
        match but {
            Button::A=> {
                self.user_button.is_low().ok().unwrap()
            }
            Button::B => {
                self.user_button.is_low().ok().unwrap()
            }
            _ => {
                self.user_button.is_low().ok().unwrap()
            }
        }
    }

}

impl<CTIMER> XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{
    fn get_status_debounced(&mut self) -> State {
        // first, remove jitter
        let mut new_state = self.state();
        self.timer.start(1_000.microseconds());
        nb::block!(self.timer.wait()).ok();
        let new_state2 = self.state();

        if new_state.a != new_state2.a {
            new_state.a = self.last_state.a;
        }
        if new_state.b != new_state2.b{
            new_state.b = self.last_state.b;
        }
        if new_state.middle != new_state2.middle{
            new_state.middle = self.last_state.middle;
        }

        new_state
    }

    fn read_button_edge(&mut self, but: Button, edge_type: bool) -> bool {

        let new_state = self.get_status_debounced();

        let mid_edge = (self.last_state.middle ^ new_state.middle) && (self.last_state.middle ^ edge_type);
        let top_edge = (self.last_state.a ^ new_state.a) && (self.last_state.a ^ edge_type);
        let bot_edge = (self.last_state.b ^ new_state.b) && (self.last_state.b ^ edge_type);

        match but {
            Button::A => {
                self.last_state.a = new_state.a;
                top_edge
            }
            Button::B => {
                self.last_state.b = new_state.b;
                bot_edge
            }
            Button::Middle => {
                self.last_state.middle = new_state.middle;
                mid_edge
            }
        }
    }
}

impl<CTIMER> Edge for XpressoButtons <CTIMER>
where CTIMER: ctimer::Ctimer<init_state::Enabled>
{
    /// Non-blockingly wait for the button to be pressed.
    /// This is edge sensitive, meaning it will not complete successfully more than once
    /// per actual button press.
    /// Use block!(...) macro to actually block.
    fn wait_for_new_press(&mut self, but: Button) -> nb::Result<(), Infallible> {
        let result = self.read_button_edge(but, true);
        if result {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// Same as for wait_for_press, but waits for the release of the button.
    fn wait_for_new_release(&mut self, but: Button) -> nb::Result<(), Infallible> {
        let result = self.read_button_edge(but, false);
        if result {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// See wait_for_press
    fn wait_for_any_new_press(&mut self, ) -> nb::Result<Button, Infallible> {
        if self.read_button_edge(Button::A, true) {
            Ok(Button::A)
        } else if self.read_button_edge(Button::B, true) {
            Ok(Button::B)
        } else if self.read_button_edge(Button::Middle, true) {
            Ok(Button::Middle)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// See wait_for_release
    fn wait_for_any_new_release(&mut self, ) -> nb::Result<Button, Infallible> {
        if self.read_button_edge(Button::A, false) {
            Ok(Button::A)
        } else if self.read_button_edge(Button::B, false) {
            Ok(Button::B)
        } else if self.read_button_edge(Button::Middle, false) {
            Ok(Button::Middle)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    fn wait_for_new_squeeze(&mut self, ) -> nb::Result<(), Infallible> {
        let oldstate = self.last_state;
        let a = self.read_button_edge(Button::A, true);
        let b = self.read_button_edge(Button::B, true);
        if a && b {
            Ok(())
        } else {
            if a { self.last_state.a = oldstate.a; }
            if b { self.last_state.b = oldstate.b; }
            Err(nb::Error::WouldBlock)
        }
    }
}
