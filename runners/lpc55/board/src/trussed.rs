//! Implementation of `trussed::Platform` for the board,
//! using the specific implementation of our `crate::traits`.

use core::time::Duration;

use crate::hal::{
    peripherals::rtc::Rtc,
    typestates::init_state,
};
use crate::traits::buttons::{Press, Edge};
use crate::traits::rgb_led::{Intensities, RgbLed};
use micromath::F32;
use trussed::platform::{consent, ui};

// Assuming there will only be one way to
// get user presence, this should be fine.
// Used for Ctaphid.keepalive message status.
static mut WAITING: bool = false;
pub struct UserPresenceStatus {}
impl UserPresenceStatus {
    pub(crate) fn set_waiting(waiting: bool) {
        unsafe { WAITING = waiting };
    }
    pub fn waiting() -> bool {
        unsafe{ WAITING }
    }
}


pub struct UserInterface<BUTTONS, RGB>
where
BUTTONS: Press + Edge,
RGB: RgbLed,
{
    rtc: Rtc<init_state::Enabled>,
    buttons: Option<BUTTONS>,
    rgb: Option<RGB>,
    status: ui::Status,
    wink_until: Duration,
}

impl<BUTTONS, RGB> UserInterface<BUTTONS, RGB>
where
BUTTONS: Press + Edge,
RGB: RgbLed,
{
    pub fn new(rtc: Rtc<init_state::Enabled>, buttons: Option<BUTTONS>, rgb: Option<RGB>) -> Self {
        #[allow(unused_mut)]
        let mut buttons = buttons;
        #[cfg(feature = "no-buttons")]
        {
            buttons = None;
        }
        Self {
            rtc, buttons, rgb,
            status: ui::Status::Idle,
            wink_until: Duration::new(0, 0),
        }
    }
}

// color codes Conor picked
const BLACK: Intensities = Intensities { red: 0, green: 0, blue: 0 };
// const RED: Intensities = Intensities { red: u8::MAX, green: 0, blue: 0 };
// const GREEN: Intensities = Intensities { red: 0, green: u8::MAX, blue: 0x02 };
const BLUE: Intensities = Intensities { red: 0, green: 0, blue: u8::MAX };
// const TEAL: Intensities = Intensities { red: 0, green: u8::MAX, blue: 0x5a };
// const ORANGE: Intensities = Intensities { red: u8::MAX, green: 0x7e, blue: 0 };

impl<BUTTONS, RGB> trussed::platform::UserInterface for UserInterface<BUTTONS,RGB>
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
                UserPresenceStatus::set_waiting(true);
                let press_result = buttons.wait_for_any_new_press();
                UserPresenceStatus::set_waiting(false);
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

        self.status = status;
        debug_now!("status set to {:?}", status);

        // self.refresh runs periodically and would overwrite this
        // if let Some(rgb) = &mut self.rgb {
        //     rgb.set(match status {
        //         ui::Status::Idle => GREEN,
        //         ui::Status::Processing => TEAL,
        //         ui::Status::WaitingForUserPresence => ORANGE,
        //         ui::Status::Error => RED,
        //     });
        // }
    }

    fn refresh(&mut self) {
        let uptime = self.uptime();

        if let Some(rgb) = self.rgb.as_mut() {
            let period = Duration::new(5, 0).as_millis() as u32;
            let tau = F32(6.283185);
            let angle = F32(uptime.as_millis() as f32) * tau / (period as f32);
            let min_amplitude: u8 = 4;
            let max_amplitude: u8 = 64;
            let rel_amplitude = max_amplitude - min_amplitude;

            // sinoidal wave on top of a baseline brightness
            let amplitude = min_amplitude + (angle.sin().abs() * (rel_amplitude as f32)).floor().0 as u8;

            let any_button = self.buttons.as_mut()
                .map(|buttons| buttons.state())
                .map(|state| state.a || state.b || state.middle)
                .unwrap_or(false);

            let mut color = if !any_button {
                // Use green if no button is pressed.
                Intensities {
                    red: 0,
                    green: amplitude,
                    blue: 0,
                }
            } else {
                // Use blue if button is pressed.
                Intensities {
                    red: 0,
                    green: 0,
                    blue: amplitude,
                }
            };
            if self.status == ui::Status::WaitingForUserPresence {
                color = BLUE;
            }
            if uptime < self.wink_until {
                let on = (((F32(uptime.as_secs_f32())*4.0f32).round().0 as u32) % 2) != 0;
                color = if on { BLUE } else { BLACK };
            }

            // use logging::hex::*;
            // use logging::hex;
            // crate::logger::info!("time: {}", time).ok();
            // debug_now!("amp: {:08X}", amplitude);
            // crate::logger::info!("color: {}", hex!(color)).ok();
            rgb.set(color.into());
        }
    }

    fn uptime(&mut self) -> Duration {
        self.rtc.uptime()
    }

    fn wink(&mut self, duration: Duration) {
        debug_now!("winking for {:?}", duration);
        self.wink_until = self.uptime() + duration;
    }

}
