use crate::hal::{
    peripherals::rtc::Rtc,
    typestates::init_state,
};
use board_traits::buttons::{Press, Edge};
use board_traits::rgb_led::RgbLed;
use trussed::board::{
    ui,
    consent,
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
        #[cfg(not(feature = "no-buttons"))]
        let ui = Self { buttons, rgb };
        #[cfg(feature = "no-buttons")]
        let ui = Self { buttons: None, rgb };

        ui
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
    fn check_user_presence(&mut self) -> consent::Level {
        match &mut self.buttons {
            Some(buttons) => {
                // important to read state before checking for edge,
                // since reading an edge could clear the state.
                let state = buttons.state();
                let press_result = buttons.wait_for_any_new_press();
                if press_result.is_ok() {
                    if state.a && state.b {
                        consent::Level::Strong
                    } else {
                        consent::Level::Normal
                    }
                } else {
                    consent::Level::None
                }
            }
            None => {
                // With configured with no buttons, that means Solo is operating
                // in passive NFC mode, which means user tapped to indicate presence.
                consent::Level::Normal
            }
        }
    }

    fn set_status(&mut self, status: ui::Status) {

        if let Some(rgb) = &mut self.rgb {

            match status {
                ui::Status::Idle => {
                    // green
                    rgb.set(0x00_ff_02.into());
                },
                ui::Status::Processing => {
                    // teal
                    rgb.set(0x00_ff_5a.into());
                }
                ui::Status::WaitingForUserPresence => {
                    // orange
                    rgb.set(0xff_7e_00.into());
                },
                ui::Status::Error => {
                    // Red
                    rgb.set(0xff_00_00.into());
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
