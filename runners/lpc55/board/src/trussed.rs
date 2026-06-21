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

/// Status-LED colors (`0x00RRGGBB`), set by the runner from the persisted
/// DeviceConfig. Defaults reproduce the stock idle-green / UP-blue. `idle` is
/// shared with Processing, `up` with wink; Error is always red.
pub static LED_IDLE_RGB: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0x0000_3F00);
pub static LED_UP_RGB: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0x0000_007F);

pub struct UserPresenceStatus {}
impl UserPresenceStatus {
    pub(crate) fn set_waiting(waiting: bool) {
        unsafe { WAITING = waiting };
    }
    pub fn waiting() -> bool {
        unsafe { WAITING }
    }
}

/// True when a wallet sign is currently waiting for user presence. The wallet
/// path waits via the runner-driven non-blocking consent (`confirm_user_present_non_blocking`),
/// which never calls trussed's `set_status`, so trussed's own `WAITING` flag
/// stays clear. The runner's idle loop mirrors `wallet_app::consent::
/// is_up_requested()` into this flag each pass (the board crate has no
/// wallet-app dep), and the LED driver (`set_status` + `refresh`) ORs it with
/// trussed's `WAITING` so the "waiting" color also lights during a wallet sign.
static UI_WAITING: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Mirror a waiting wallet sign into the LED driver (called from idle).
pub fn set_wallet_up_requested(requested: bool) {
    UI_WAITING.store(requested, core::sync::atomic::Ordering::Relaxed);
}

/// True when *any* source wants the UP indicator: trussed's status
/// (FIDO etc.) or a waiting wallet sign.
fn up_indicator_wanted(status: ui::Status) -> bool {
    matches!(status, ui::Status::WaitingForUserPresence)
        || UI_WAITING.load(core::sync::atomic::Ordering::Relaxed)
}

// ── Latched button-gesture global ────────────────────────────────────────────
//
// Buttons are hoisted out of `UserInterface` so the idle loop can poll them.
// `poll_buttons` (called from idle) latches a committed gesture here; both
// `check_user_presence` (FIDO, via trussed's UP loop) and the runner's
// `confirm_user_present_non_blocking` (wallet) drain it via `take_gesture` (one-shot). They never
// run concurrently (one consent at a time), so a single latch suffices.

use core::sync::atomic::AtomicU8;

const GESTURE_NONE: u8 = 0;
const GESTURE_APPROVE: u8 = 1;
const GESTURE_STRONG: u8 = 2;

static BUTTON_GESTURE: AtomicU8 = AtomicU8::new(GESTURE_NONE);

/// Committed button gesture. lpc55 has no explicit-deny button, so the only
/// outcomes are an approve (single press) or a strong approve (A+B squeeze).
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Gesture {
    None,
    Approve,
    Strong,
}

fn latch_gesture(g: Gesture) {
    use core::sync::atomic::Ordering;
    match g {
        // Don't downgrade a latched Strong to Approve before it's consumed.
        Gesture::Approve => {
            let _ = BUTTON_GESTURE.compare_exchange(
                GESTURE_NONE,
                GESTURE_APPROVE,
                Ordering::Release,
                Ordering::Relaxed,
            );
        }
        Gesture::Strong => BUTTON_GESTURE.store(GESTURE_STRONG, Ordering::Release),
        Gesture::None => {}
    }
}

/// One-shot consume: read the latched gesture and reset to None.
pub fn take_gesture() -> Gesture {
    use core::sync::atomic::Ordering;
    match BUTTON_GESTURE.swap(GESTURE_NONE, Ordering::AcqRel) {
        GESTURE_APPROVE => Gesture::Approve,
        GESTURE_STRONG => Gesture::Strong,
        _ => Gesture::None,
    }
}

// ── Shared user-presence INPUT read ──────────────────────────────────────────
//
// The single place that reads the idle-reachable presence inputs (the test
// UP_CONTROL override, the NFC field via `FIDO_OVER_NFC`, and the latched
// button gesture). Both the blocking trussed `check_user_presence` and the
// runner's non-blocking `confirm_user_present_non_blocking` wrap this; neither
// touches the LED — that lives in `set_status` / `refresh`.
//
// lpc55 has no explicit-deny button, so the gesture only yields Approve/Strong;
// there is no `Deny` arm.
//
// `has_buttons`: a no-buttons build/passive boot auto-approves UP as `Strong`
// (so a Strong-gated Reset still works). `nfc_suppressed`: the non-blocking
// wallet path suppresses an NFC grant while an NDEF override URL is being read
// (a phone tap during a sign); the blocking FIDO path passes `false`.
pub enum Presence {
    Grant(consent::Level),
    Pending,
}

pub fn read_presence(has_buttons: bool, nfc_suppressed: bool) -> Presence {
    #[cfg(feature = "test-up-control")]
    {
        let val = unsafe { core::ptr::read_volatile(&raw const UP_CONTROL) };
        match val {
            1 => {
                unsafe { core::ptr::write_volatile(&raw mut UP_CONTROL, 0) };
                return Presence::Grant(consent::Level::Normal);
            }
            2 => {
                unsafe { core::ptr::write_volatile(&raw mut UP_CONTROL, 0) };
                return Presence::Grant(consent::Level::Strong);
            }
            129 => return Presence::Grant(consent::Level::Normal),
            130 => return Presence::Grant(consent::Level::Strong),
            // 0 / unknown / uninitialized: fall through to real buttons.
            _ => {}
        }
    }

    // NFC request: the tap reaching the card IS the user presence. The wallet
    // path suppresses it while an NDEF override URL is being read.
    if FIDO_OVER_NFC.load(core::sync::atomic::Ordering::Relaxed) && !nfc_suppressed {
        return Presence::Grant(consent::Level::Normal);
    }

    if !has_buttons {
        // no-buttons builds (auto-approve UP for tests + NFC mode). Strong so
        // it satisfies both Normal-gated ops and the Strong-gated Reset.
        return Presence::Grant(consent::Level::Strong);
    }

    match take_gesture() {
        Gesture::Strong => Presence::Grant(consent::Level::Strong),
        Gesture::Approve => Presence::Grant(consent::Level::Normal),
        Gesture::None => Presence::Pending,
    }
}

/// Idle-loop button poll: read the buttons, and on a fresh press latch an
/// Approve (or Strong on an A+B squeeze). Edge-sensitive via `Edge`, so it
/// returns Ok only once per press. lpc55 buttons are fast GPIO/cap reads, so
/// no throttling is needed. Runs every idle pass — FIDO needs fresh gestures
/// too, not just the wallet path.
pub fn poll_buttons<B: Press + Edge>(buttons: &mut B) {
    // Read state before the edge check — reading an edge can clear the state.
    let state = buttons.state();
    if buttons.wait_for_any_new_press().is_ok() {
        if state.a && state.b {
            latch_gesture(Gesture::Strong);
        } else {
            latch_gesture(Gesture::Approve);
        }
    }
}

/// Transport of the FIDO request currently being dispatched: `true` = NFC
/// (contactless), `false` = USB/contact. The runner's dispatch sets it before
/// fido handles a request; `check_user_presence` reads it so an NFC request
/// takes the tap as presence while USB still requires a button press.
pub static FIDO_OVER_NFC: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Whether this build has physical buttons. The buttons live in the idle loop
/// (see `poll_buttons`); the UI keeps only this flag so `no-buttons`
/// builds (NFC / test auto-approve) can still return `Strong` from
/// `check_user_presence`.
pub struct UserInterface<RGB>
where
    RGB: RgbLed,
{
    rtc: Rtc<init_state::Enabled>,
    has_buttons: bool,
    rgb: Option<RGB>,
    status: ui::Status,
    wink_until: Duration,
}

impl<RGB> UserInterface<RGB>
where
    RGB: RgbLed,
{
    pub fn new(rtc: Rtc<init_state::Enabled>, has_buttons: bool, rgb: Option<RGB>) -> Self {
        // no-buttons builds auto-approve UP regardless of physical presence.
        #[cfg(feature = "no-buttons")]
        let has_buttons = {
            let _ = has_buttons;
            false
        };
        Self {
            rtc,
            has_buttons,
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
    green: 0x3F,
    blue: 0x02,
};
const BLUE: Intensities = Intensities {
    red: 0,
    green: 0,
    blue: 0x7F,
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

impl<RGB> trussed::platform::UserInterface for UserInterface<RGB>
where
    RGB: RgbLed,
{
    fn check_user_presence(&mut self) -> consent::Level {
        // Thin wrapper over the shared `read_presence`. The blocking FIDO path
        // never suppresses an NFC grant (the wallet path does), so pass
        // `nfc_suppressed = false`. WAITING is driven by set_status() so the
        // periodic keepalive task observes UP_NEEDED for the full UP-wait
        // window.
        match read_presence(self.has_buttons, false) {
            Presence::Grant(level) => level,
            Presence::Pending => consent::Level::None,
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
            // UP indicator also lights for a waiting wallet sign (UI_WAITING),
            // not just trussed's WaitingForUserPresence.
            if up_indicator_wanted(status) {
                rgb.set(BLUE);
            } else {
                rgb.set(match status {
                    ui::Status::Idle => GREEN,
                    // ui::Status::Idle => RED,
                    ui::Status::Processing => TEAL,
                    // ui::Status::Processing => GREEN,
                    ui::Status::Error => RED,
                    _ => BLACK,
                });
            }
        }
    }

    fn status(&self) -> ui::Status {
        self.status
    }

    fn refresh(&mut self) {
        let uptime = self.uptime().as_millis() as u32;

        if let Some(rgb) = self.rgb.as_mut() {
            // UP indicator also breathes for a waiting wallet sign (UI_WAITING),
            // not just trussed's WaitingForUserPresence.
            let waiting_for_user = up_indicator_wanted(self.status);
            let processing = self.status == ui::Status::Processing;
            let winking = uptime < self.wink_until.as_millis() as u32;
            // Colors come from the persisted DeviceConfig (set by the runner):
            // idle is shared with Processing, up with wink; Error stays red.
            let idle = LED_IDLE_RGB.load(core::sync::atomic::Ordering::Relaxed);
            let up = LED_UP_RGB.load(core::sync::atomic::Ordering::Relaxed);
            let full = |c: u32| Intensities {
                red: (c >> 16) as u8,
                green: (c >> 8) as u8,
                blue: c as u8,
            };
            let scaled = |c: u32, amp: u8| {
                let ch = |shift: u32| (((c >> shift) as u8 as u16 * amp as u16) / 255) as u8;
                Intensities {
                    red: ch(16),
                    green: ch(8),
                    blue: ch(0),
                }
            };
            let blink_on = || !((F32(uptime as f32) / 250.0).round().0 as u32).is_multiple_of(2);

            let color = if waiting_for_user {
                // breathe fast, in the UP color
                scaled(up, calculate_amplitude(uptime, 2, 4, 75))
            } else if processing {
                // blink in the idle color
                if blink_on() {
                    full(idle)
                } else {
                    BLACK
                }
            } else if winking {
                // blink rapidly in the UP color
                if blink_on() {
                    full(up)
                } else {
                    BLACK
                }
            } else {
                // breathe slowly in the idle color
                scaled(idle, calculate_amplitude(uptime, 10, 4, 64))
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
