use crate::hal::{
    peripherals::rtc::Rtc,
    typestates::init_state,
};
use board_traits::buttons::{Press, Edge};
use board_traits::rgb_led::RgbLed;
use trussed::board::{
    UserPresenceIndication,
    Status,
};

pub struct UserInterface<BUTTONS, RGB>
where
BUTTONS: Press + Edge,
RGB: RgbLed,
{
    buttons: Option<BUTTONS>,
    rgb: Option<RGB>,
}

impl<BUTTONS, RGB> UserInterface<BUTTONS, RGB>
where
BUTTONS: Press + Edge,
RGB: RgbLed,
{
    pub fn new(buttons: Option<BUTTONS>, rgb: Option<RGB>) -> Self {
        Self { buttons, rgb }
    }
}

pub struct UpTime {
    rtc: Rtc<init_state::Enabled>
}

impl UpTime {
    pub fn new(rtc: Rtc<init_state::Enabled>) -> Self {
        Self { rtc }
    }
}


impl<BUTTONS, RGB> trussed::board::UserInterface for UserInterface<BUTTONS,RGB>
where
BUTTONS: Press + Edge,
RGB: RgbLed,
{
    fn check_user_presence(&mut self) -> UserPresenceIndication {
        match &mut self.buttons {
            Some(buttons) => {
                // important to read state before checking for edge,
                // since reading an edge could clear the state.
                let state = buttons.state();
                let press_result = buttons.wait_for_any_new_press();
                if press_result.is_ok() {
                    if state.a && state.b {
                        UserPresenceIndication::Strong
                    } else {
                        UserPresenceIndication::Light
                    }
                } else {
                    UserPresenceIndication::None
                }
            }
            None => {
                UserPresenceIndication::CantTell
            }
        }
    }

    fn set_status(&mut self, status: Status) {

        if let Some(rgb) = &mut self.rgb {

            match status {
                Status::Idle => {
                    // greenish
                    rgb.set(0x015002.into());
                },
                Status::Processing => {
                    // tealish
                    rgb.set(0x010152.into());

                }
                Status::WaitingForUserPresence => {
                    // Orange
                    rgb.set(0x411112.into());
                },
                Status::Error => {
                    // Red
                    rgb.set(0x510101.into());
                },
            }

        }
    }
}

impl trussed::board::UpTime for UpTime {
    fn uptime(&mut self) -> core::time::Duration {
        self.rtc.uptime()
    }
}
