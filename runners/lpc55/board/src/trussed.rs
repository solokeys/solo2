//! Implementation of `trussed::Platform` for the board,
//! using the specific implementation of our `crate::traits`.

use core::time::Duration;

use crate::hal::{peripherals::rtc::Rtc, typestates::init_state};
use crate::traits::buttons::{Edge, Press};
use crate::traits::rgb_led::{Intensities, RgbLed};
use defmt::debug;
use micromath::F32;
use trussed::platform::{consent, ui};

// Assuming there will only be one way to
// get user presence, this should be fine.
// Used for Ctaphid.keepalive message status.
static mut WAITING: bool = false;

/// Probe-rs-writable user-presence override for automated tests.
///
/// Value semantics (see the match in `check_user_presence` below):
///   1   = approve once (Normal), consumed after one read
///   2   = approve once (Strong), consumed after one read
///   129 = approve sticky (Normal) until reset to 0
///   130 = approve sticky (Strong) until reset to 0
///   0 / any other value (incl. uninitialized) = no override; falls
///         through to real button polling. A host "deny" is expressed
///         this way (no tap within the window = timeout).
///
/// Placed in `.uninit` with `no_mangle` so its address is stable and
/// discoverable via the ELF symbol table.
#[cfg(feature = "test-up-control")]
#[unsafe(link_section = ".uninit")]
#[unsafe(no_mangle)]
pub static mut UP_CONTROL: u8 = 0;

pub struct UserPresenceStatus {}
impl UserPresenceStatus {
    pub(crate) fn set_waiting(waiting: bool) {
        unsafe { WAITING = waiting };
    }
    pub fn waiting() -> bool {
        unsafe { WAITING }
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
    pub fn new(rtc: Rtc<init_state::Enabled>, _buttons: Option<BUTTONS>, rgb: Option<RGB>) -> Self {
        #[allow(unused_mut)]
        let mut buttons = {
            #[cfg(not(feature = "no-buttons"))]
            {
                _buttons
            }
            #[cfg(feature = "no-buttons")]
            {
                None
            }
        };
        Self {
            rtc,
            buttons,
            rgb,
            status: ui::Status::Idle,
            wink_until: Duration::new(0, 0),
        }
    }
}

// color codes Conor picked
const BLACK: Intensities = Intensities {
    red: 0,
    green: 0,
    blue: 0,
};
const RED: Intensities = Intensities {
    red: u8::MAX,
    green: 0,
    blue: 0,
};
const GREEN: Intensities = Intensities {
    red: 0,
    green: 15,
    blue: 0x02,
};
const BLUE: Intensities = Intensities {
    red: 0,
    green: 0,
    blue: 55,
};
const TEAL: Intensities = Intensities {
    red: 0,
    green: 55,
    blue: 20,
};
#[allow(dead_code)]
const ORANGE: Intensities = Intensities {
    red: u8::MAX,
    green: 0x7e,
    blue: 0,
};
#[allow(dead_code)]
const WHITE: Intensities = Intensities {
    red: u8::MAX,
    green: u8::MAX,
    blue: u8::MAX,
};

impl<BUTTONS, RGB> trussed::platform::UserInterface for UserInterface<BUTTONS, RGB>
where
    BUTTONS: Press + Edge,
    RGB: RgbLed,
{
    fn check_user_presence(&mut self) -> consent::Level {
        // Probe-rs UP override: write `UP_CONTROL` from the host (via
        // probe-rs) to drive automated tests. See the static's docs above
        // for the value mapping. One-shot values (1, 2) are consumed.
        // The JTAG override is additive: it can only *grant* user presence.
        // Any value that is not a recognized approve marker (including 0 and
        // uninitialized garbage) falls through to real button polling, so the
        // physical buttons always work.
        #[cfg(feature = "test-up-control")]
        {
            let val = unsafe { core::ptr::read_volatile(&raw const UP_CONTROL) };
            match val {
                1 => {
                    unsafe { core::ptr::write_volatile(&raw mut UP_CONTROL, 0) };
                    return consent::Level::Normal;
                }
                2 => {
                    unsafe { core::ptr::write_volatile(&raw mut UP_CONTROL, 0) };
                    return consent::Level::Strong;
                }
                129 => return consent::Level::Normal,
                // Strong sticky — required to drive CTAP 2.3 long-touch
                // Reset from a host that can't pulse the UP byte per
                // call (the only consent level that satisfies both
                // `user_present` and `user_present_strong`).
                130 => return consent::Level::Strong,
                // 0 / unknown / uninitialized: fall through to real buttons.
                _ => {}
            }
        }

        match &mut self.buttons {
            Some(buttons) => {
                // important to read state before checking for edge,
                // since reading an edge could clear the state.
                let state = buttons.state();
                // WAITING is driven by set_status() so the periodic keepalive
                // task observes UP_NEEDED for the full UP-wait window.
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
                // no-buttons builds (auto-approve UP for tests + NFC mode).
                // Return Strong so it satisfies both Normal-gated ops and the
                // Strong-gated Reset; Normal would block Reset entirely.
                consent::Level::Strong
            }
        }
    }

    fn set_status(&mut self, status: ui::Status) {
        self.status = status;
        // Drive the static WAITING flag from the trussed status so the
        // periodic CTAPHID keepalive task emits STATUS_UPNEEDED for the
        // entire UP-wait window (CTAP §11.2.9.1.2).
        UserPresenceStatus::set_waiting(matches!(status, ui::Status::WaitingForUserPresence));
        debug!("status set to {:?}", defmt::Debug2Format(&status));

        // self.refresh runs periodically and would overwrite this
        if let Some(rgb) = &mut self.rgb {
            rgb.set(match status {
                ui::Status::Idle => GREEN,
                // ui::Status::Idle => RED,
                ui::Status::Processing => TEAL,
                // ui::Status::Processing => GREEN,
                // ui::Status::WaitingForUserPresence => ORANGE,
                ui::Status::WaitingForUserPresence => BLUE,
                ui::Status::Error => RED,
                _ => BLACK,
            });
        }
    }

    fn status(&self) -> ui::Status {
        self.status
    }

    fn refresh(&mut self) {
        let uptime = self.uptime().as_millis() as u32;

        if let Some(rgb) = self.rgb.as_mut() {
            let waiting_for_user = self.status == ui::Status::WaitingForUserPresence;
            let processing = self.status == ui::Status::Processing;
            let winking = uptime < self.wink_until.as_millis() as u32;
            let any_button = self
                .buttons
                .as_mut()
                .map(|buttons| buttons.state())
                .map(|state| state.a || state.b || state.middle)
                .unwrap_or(false);

            let color = if waiting_for_user {
                // breathe fast, in blue

                let amplitude = calculate_amplitude(uptime, 2, 4, 75);
                Intensities {
                    red: 0,
                    green: 0,
                    blue: amplitude,
                }
            } else if processing {
                let on = !((F32(uptime as f32) / 250.0).round().0 as u32).is_multiple_of(2);
                if on {
                    GREEN
                } else {
                    BLACK
                }
            } else if winking {
                // blink rapidly

                let on = !((F32(uptime as f32) / 250.0).round().0 as u32).is_multiple_of(2);
                if on {
                    BLUE
                } else {
                    BLACK
                }
                // if on { WHITE } else { BLACK }
            } else {
                // regular behaviour: breathe slowly

                let amplitude = calculate_amplitude(uptime, 10, 4, 64);

                if !any_button {
                    // Use green if no button is pressed.
                    Intensities {
                        red: 0,
                        green: amplitude,
                        blue: 0,
                    }
                    // Intensities { red: amplitude, green: 0, blue: 0 }
                } else {
                    // Use blue if button is pressed.
                    Intensities {
                        red: 0,
                        green: 0,
                        blue: amplitude,
                    }
                }
            };

            // use logging::hex::*;
            // use logging::hex;
            // crate::logger::info!("time: {}", time).ok();
            // debug_now!("amp: {:08X}", amplitude);
            // crate::logger::info!("color: {}", hex!(color)).ok();
            rgb.set(color);
        }
    }

    fn uptime(&mut self) -> Duration {
        self.rtc.uptime()
    }

    fn wink(&mut self, duration: Duration) {
        debug!("winking for {:?}", duration);
        self.wink_until = self.uptime() + duration;
    }
}

fn calculate_amplitude(
    now_millis: u32,
    period_secs: u8,
    min_amplitude: u8,
    max_amplitude: u8,
) -> u8 {
    let period = Duration::new(period_secs as u64, 0).as_millis() as u32;
    let tau = F32(core::f32::consts::TAU);
    let angle = F32(now_millis as f32) * tau / (period as f32);
    let rel_amplitude = max_amplitude - min_amplitude;

    // sinoidal wave on top of a baseline brightness

    min_amplitude + (angle.sin().abs() * (rel_amplitude as f32)).floor().0 as u8
}
