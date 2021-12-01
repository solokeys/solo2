//! Implementation of `trussed::Platform` for the board,
//! using the specific implementation of our `crate::traits`.

use core::time::Duration;

use crate::hal::{
    peripherals::rtc::Rtc,
    typestates::init_state,
};
use crate::traits::buttons::{Press, Edge};
use crate::traits::rgb_led::RgbLed;
use trussed::platform::{
    ui,
    reboot,
    consent,
};

// translated from https://stackoverflow.com/a/2284929/2490057
fn sin(x: f32) -> f32
{

    let mut res = 0f32;
    let mut pow = x;
    let mut fact = 1f32;
    for i in 0..5 {
        res += pow/fact;
        pow *= -1f32 * x * x;
        fact *= ((2*(i+1))*(2*(i+1)+1)) as f32;
    }

    res
}

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
    wink: Option<core::ops::Range<Duration>>,
    provisioner: bool,
}

impl<BUTTONS, RGB> UserInterface<BUTTONS, RGB>
where
BUTTONS: Press + Edge,
RGB: RgbLed,
{
    pub fn new(
        rtc: Rtc<init_state::Enabled>,
        _buttons: Option<BUTTONS>,
        rgb: Option<RGB>,
        provisioner: bool,
    ) -> Self {
        let wink = None;
        #[cfg(not(feature = "no-buttons"))]
        let ui = Self { rtc, buttons: _buttons, rgb, wink, provisioner };
        #[cfg(feature = "no-buttons")]
        let ui = Self { rtc, buttons: None, rgb, wink, provisioner };

        ui
    }
}

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
        if let Some(rgb) = &mut self.rgb {

            match status {
                ui::Status::Idle => {
                    if self.provisioner {
                        // white
                        rgb.set(0xff_ff_ff.into());
                    } else {
                        // green
                        rgb.set(0x00_ff_02.into());
                    }
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

        // Abort winking if the device is no longer idle
        if status != ui::Status::Idle {
            self.wink = None;
        }
    }

    fn refresh(&mut self) {
        if self.rgb.is_none() {
            return;
        }

        if let Some(wink) = self.wink.clone() {
            let time = self.uptime();
            if wink.contains(&time) {
                // 250 ms white, 250 ms off
                let color = if (time - wink.start).as_millis() % 500 < 250 {
                    0xff_ff_ff
                } else {
                    0x00_00_00
                };
                self.rgb.as_mut().unwrap().set(color.into());
                return;
            } else {
                self.wink = None;
            }
        }

        if self.buttons.is_some() {
            // 1. Get time & pick a period (here 4096).
            // 2. Map it to a value between 0 and pi.
            // 3. Calculate sine and map to amplitude between 0 and 255.
            let time = (self.uptime().as_millis()) % 4096;
            let amplitude = (sin((time as f32) * 3.14159265f32/4096f32) * 255f32) as u32;

            let state = self.buttons.as_mut().unwrap().state();
            let color = if state.a || state.b || state.middle {
                // Use blue if button is pressed.
                0x00_00_01 | (amplitude << 0)
            } else {
                // Use green if no button is pressed.
                0x00_00_01 | (amplitude << 8)
            };
            // use logging::hex::*;
            // use logging::hex;
            // crate::logger::info!("time: {}", time).ok();
            // crate::logger::info!("amp: {}", hex!(amplitude)).ok();
            // crate::logger::info!("color: {}", hex!(color)).ok();
            self.rgb.as_mut().unwrap().set(color.into());
        }
    }

    fn uptime(&mut self) -> Duration {
        self.rtc.uptime()
    }

    // delete this function after trussed is updated
    fn reboot(&mut self, _to: reboot::To) -> ! {
        panic!("this should no longer be called.");
    }

    fn wink(&mut self, duration: Duration) {
        let time = self.uptime();
        self.wink = Some(time..time + duration);
        self.rgb.as_mut().unwrap().set(0xff_ff_ff.into());
    }
}
