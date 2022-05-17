//! Implementation of `trussed::Platform` for the board,
//! using the specific implementation of our `crate::traits`.

use core::time::Duration;


use crate::traits::{
	buttons::{Press, Button},
	rgb_led::{
        RgbLed,
        GREEN, WHITE, TEAL, ORANGE, RED, BLACK, BLUE
    }

};
use trussed::platform::{
    ui, consent,
};

use crate::runtime::UserPresenceStatus;


use rtic::Monotonic;
use embedded_time::duration::*;
type RtcMonotonic = crate::soc::rtic_monotonic::RtcMonotonic;
type RtcInstant = crate::soc::rtic_monotonic::RtcInstant;

pub struct UserInterface<BUTTONS, RGB>
where
BUTTONS: Press,
RGB: RgbLed,
{
    buttons: Option<BUTTONS>,
    rgb: Option<RGB>,
    wink: Option<core::ops::Range<Duration>>,
    provisioner: bool,
    rtc_mono: RtcMonotonic,
}

impl<BUTTONS, RGB> UserInterface<BUTTONS, RGB>
where
BUTTONS: Press,
RGB: RgbLed,
{
    pub fn new(
        _buttons: Option<BUTTONS>,
        rgb: Option<RGB>,
        provisioner: bool
    ) -> Self {
        let wink = None;
        let pac = unsafe { nrf52840_pac::Peripherals::steal() };
        let rtc_mono = RtcMonotonic::new(pac.RTC0);

        #[cfg(not(feature = "no-buttons"))]
        let ui = Self { buttons: _buttons, rgb, wink, provisioner, rtc_mono };
        #[cfg(feature = "no-buttons")]
        let ui = Self { buttons: None, rgb, wink, provisioner, rtc_mono };

        ui
    }
}

impl<BUTTONS, RGB> trussed::platform::UserInterface for UserInterface<BUTTONS,RGB>
where
BUTTONS: Press,
RGB: RgbLed,
{
    fn check_user_presence(&mut self) -> consent::Level {
        // essentially a blocking call for up to ~30secs
        // this outer loop accumulates *presses* from the
        // inner loop & maintains (loading) delays.

        // no buttons configured -> always consent
        if self.buttons.is_none() {
            return consent::Level::Normal;
        }

        let mut counter: u8 = 0;
        let mut is_pressed = false;
        let threshold: u8 = 1;

        let start_time = self.uptime().as_millis();
        let timeout_at = start_time + 28_000u128;
        let mut next_check = start_time + 25u128;

        self.set_status(ui::Status::WaitingForUserPresence);

        loop {
            let cur_time = self.uptime().as_millis();

            // timeout reached
            if cur_time > timeout_at {
                break;
            }
            // loop until next check shall be done
            if cur_time < next_check {
                continue;
            }

            if let Some(button) = self.buttons.as_mut() {
                UserPresenceStatus::set_waiting(true);
                is_pressed = button.is_pressed(Button::A);
                UserPresenceStatus::set_waiting(false);
            }

            if is_pressed {
                counter += 1;
                // with press -> delay 25ms
                next_check = cur_time + 25;

                // during press set LED to blue
                if let Some(rgb) = &mut self.rgb {
                    rgb.set(BLUE.into());
                }
            } else {
                // w/o press -> delay 100ms
                next_check = cur_time + 100;
            }

            if counter >= threshold {
                break;
            }
        }

        // consent, if we've counted 3 "presses"
        if counter >= threshold {
            consent::Level::Normal
        } else {
            consent::Level::None
        }
    }

    fn set_status(&mut self, status: ui::Status) {
        if let Some(rgb) = &mut self.rgb {

            match status {
                ui::Status::Idle => {
                    if self.provisioner {
                        rgb.set(WHITE.into());
                    } else {
                        rgb.set(GREEN.into());
                    }

                },
                ui::Status::Processing => {
                    rgb.set(TEAL.into());
                }
                ui::Status::WaitingForUserPresence => {
                    rgb.set(ORANGE.into());
                },
                ui::Status::Error => {
                    rgb.set(RED.into());
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
                    WHITE
                } else {
                    BLACK
                };
                self.rgb.as_mut().unwrap().set(color.into());
                return;
            } else {
                self.wink = None;
            }
        } else {
            self.set_status(ui::Status::Idle);
        }

        /*if self.buttons.is_some() {

            // 1. Get time & pick a period (here 4096).
            // 2. Map it to a value between 0 and pi.
            // 3. Calculate sine and map to amplitude between 0 and 255.
            //let time = (self.uptime().as_millis()) % 4096;
            //let amplitude = (sin((time as f32) * 3.14159265f32/4096f32) * 255f32) as u32;

            let state = self.buttons.as_mut().unwrap().state();
            let color = if state.a || state.b || state.middle {
                // Use blue if button is pressed.
                0x00_00_01 | (amplitude << 0)
            } else {
                // Use green if no button is pressed.
                0x00_00_01 | (amplitude << 8)
            };
            let color = 0x00_00_01 ;

            // use logging::hex::*;
            // use logging::hex;
            // crate::logger::info!("time: {}", time).ok();
            // crate::logger::info!("amp: {}", hex!(amplitude)).ok();
            // crate::logger::info!("color: {}", hex!(color)).ok();
            self.rgb.as_mut().unwrap().set(color.into());

            self.set_status(ui::Status::Idle);
        }*/

    }

    fn uptime(&mut self) -> Duration {
        let ms: embedded_time::duration::units::Milliseconds = self.rtc_mono.now().into();
        core::time::Duration::from_millis(ms.integer().into())
    }

    fn wink(&mut self, duration: Duration) {
        let time = self.uptime();
        self.wink = Some(time..time + duration);
        self.rgb.as_mut().unwrap().set(WHITE.into());
    }
}
