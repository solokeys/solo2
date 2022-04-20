//! Implementation of `trussed::Platform` for the board,
//! using the specific implementation of our `crate::traits`.

use core::time::Duration;


use crate::traits::{
	buttons::{Press, Button},
	rgb_led::RgbLed,

};
use trussed::platform::{
    ui, reboot, consent,
};
use nrf52840_hal::Timer;

use crate::runtime::UserPresenceStatus;

use nrf52840_hal::prelude::_embedded_hal_blocking_delay_DelayMs;


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



pub struct UserInterface<BUTTONS, RGB>
where
BUTTONS: Press,
RGB: RgbLed,
{
    buttons: Option<BUTTONS>,
    rgb: Option<RGB>,
    wink: Option<core::ops::Range<Duration>>,
    provisioner: bool,
    delay_timer: Timer::<nrf52840_pac::TIMER0>
}

impl<BUTTONS, RGB> UserInterface<BUTTONS, RGB>
where
BUTTONS: Press,
RGB: RgbLed,
{
    pub fn new(
        _buttons: Option<BUTTONS>,
        rgb: Option<RGB>,
        provisioner: bool,
        delay_timer: Timer::<nrf52840_pac::TIMER0>
    ) -> Self {
        let wink = None;
        #[cfg(not(feature = "no-buttons"))]
        let ui = Self { buttons: _buttons, rgb, wink, provisioner, delay_timer };
        #[cfg(feature = "no-buttons")]
        let ui = Self { buttons: None, rgb, wink, provisioner, delay_timer};

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

        let mut counter: u8 = 0;
        let mut is_pressed = false;
        let max_tries: u8 = 100;
        const threshold: u8 = 3;

        for idx in 0..max_tries {

            self.delay_timer.delay_ms(25u32);

            if let Some(mut button) = self.buttons.as_mut() {
                UserPresenceStatus::set_waiting(true);
                is_pressed = button.is_pressed(Button::A);
                UserPresenceStatus::set_waiting(false);
            }

            if is_pressed {
                counter += 1;

            } else {
                // w/o press -> delay to worst-case 30secs (timeout)
                self.delay_timer.delay_ms(300u32);
            }

            if counter >= threshold {
                break;
            }
        }

        // @todo: when and how a "strong" consent?
        // consent::Level::Strong
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
            let color = 0x00_00_01 ;

            // use logging::hex::*;
            // use logging::hex;
            // crate::logger::info!("time: {}", time).ok();
            // crate::logger::info!("amp: {}", hex!(amplitude)).ok();
            // crate::logger::info!("color: {}", hex!(color)).ok();
            self.rgb.as_mut().unwrap().set(color.into());
        }
    }

    fn uptime(&mut self) -> Duration {
        let cyccnt = cortex_m::peripheral::DWT::cycle_count();
		core::time::Duration::new((cyccnt as u64) / 32_000, (cyccnt / 32) % 1_000)
    }

    fn wink(&mut self, duration: Duration) {
        let time = self.uptime();
        self.wink = Some(time..time + duration);
        self.rgb.as_mut().unwrap().set(0xff_ff_ff.into());
    }
}
