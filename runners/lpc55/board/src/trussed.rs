//! Implementation of `trussed::Platform` for the board,
//! using the specific implementation of our `crate::traits`.

use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
    time::Duration,
};

use crate::hal::{peripherals::rtc::Rtc, typestates::init_state};
use crate::traits::buttons::{Edge, Press};
use crate::traits::rgb_led::{Intensities, RgbLed};
use crate::ThreeButtons;
use critical_section::Mutex;
use defmt::debug;
use micromath::F32;
use trussed::types::ui;
use trussed_core::types::consent;

// Used for Ctaphid.keepalive message status.
static WAITING: AtomicBool = AtomicBool::new(false);

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
        WAITING.store(waiting, Ordering::Release);
    }
    pub fn waiting() -> bool {
        WAITING.load(Ordering::Acquire)
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
fn up_indicator_wanted() -> bool {
    UserPresenceStatus::waiting() || UI_WAITING.load(Ordering::Relaxed)
}

// ── Latched button-gesture global ────────────────────────────────────────────
//
// The button hardware has one owner protected by a critical-section mutex.
// Idle polls it for non-blocking wallet consent, while `check_user_presence`
// polls it from Trussed's blocking UP loop. The latter runs above idle's RTIC
// priority, so idle-only polling cannot service FIDO while UP is pending.
static BUTTONS: Mutex<RefCell<Option<ThreeButtons>>> = Mutex::new(RefCell::new(None));

const GESTURE_NONE: u8 = 0;
const GESTURE_APPROVE: u8 = 1;
const GESTURE_STRONG: u8 = 2;

static BUTTON_GESTURE: AtomicU8 = AtomicU8::new(GESTURE_NONE);
static BUTTON_ARMED: AtomicBool = AtomicBool::new(false);

/// Install the physical buttons once during board initialization.
pub fn install_buttons(buttons: Option<ThreeButtons>) {
    critical_section::with(|cs| {
        *BUTTONS.borrow(cs).borrow_mut() = buttons;
    });
}

/// Poll the physical buttons and latch any newly detected gesture.
pub fn poll_buttons() {
    critical_section::with(|cs| {
        if let Some(buttons) = BUTTONS.borrow(cs).borrow_mut().as_mut() {
            poll_buttons_inner(buttons);
        }
    });
}

/// Start a new presence window. A release must be observed before a press can
/// grant it, so a finger held from an earlier operation is not accepted.
pub fn begin_button_request() {
    BUTTON_GESTURE.store(GESTURE_NONE, Ordering::Release);
    BUTTON_ARMED.store(false, Ordering::Release);
}

/// Committed button gesture. lpc55 has no explicit-deny button, so the only
/// outcomes are an approve (single press) or a strong approve (A+B squeeze).
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Gesture {
    None,
    Approve,
    Strong,
}

fn latch_gesture(g: Gesture) {
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

/// Read the buttons and latch Approve, or Strong on an A+B squeeze.
fn poll_buttons_inner<B: Press + Edge>(buttons: &mut B) {
    // The GPIO edge tracker detects a press as a transition between two
    // consecutive calls, so it must be fed on EVERY poll — including while
    // released — not gated behind the pressed check below. (Debounced read,
    // ~1 ms per polled button.)
    #[cfg(feature = "lpcxpresso55")]
    let edge = buttons.wait_for_any_new_press().is_ok();

    let state = buttons.state();
    let pressed = state.a || state.b || state.middle;

    if !pressed {
        BUTTON_ARMED.store(true, Ordering::Release);
        return;
    }

    if !BUTTON_ARMED.load(Ordering::Acquire) {
        return;
    }

    // Solo 2's current state is already filtered by the touch driver's
    // moving-average/confidence check. Unlike its edge result, the active
    // level remains available for the duration of a human touch.
    #[cfg(feature = "solo2")]
    let accepted = true;
    // The development boards use GPIO buttons, whose edge implementation
    // provides their debounce.
    #[cfg(feature = "lpcxpresso55")]
    let accepted = edge;

    // Stay armed while held: a Strong-gated request (Reset) needs the second
    // finger of a squeeze to upgrade an already-consumed Approve, and the
    // fingers of a squeeze land further apart than one poll. Held-finger
    // protection across requests comes from `begin_button_request` disarming.
    if accepted {
        latch_gesture(if state.a && state.b {
            Gesture::Strong
        } else {
            Gesture::Approve
        });
    }
}

/// Transport of the FIDO request currently being dispatched: `true` = NFC
/// (contactless), `false` = USB/contact. The runner's dispatch sets it before
/// fido handles a request; `check_user_presence` reads it so an NFC request
/// takes the tap as presence while USB still requires a button press.
pub static FIDO_OVER_NFC: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// The buttons live in the shared `BUTTONS` mutex (see `install_buttons`),
/// polled from both trussed's blocking UP loop and idle's wallet consent; the
/// UI keeps only `has_buttons` so `no-buttons` builds (NFC / test
/// auto-approve) can still return `Strong` from `check_user_presence`.
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
        if self.has_buttons {
            poll_buttons();
        }
        // The UP wait blocks inside trussed, so `update_ui` (priority 1) cannot
        // take the lock and `refresh` never runs — without this the indicator
        // stays on the flat colour `set_status` wrote. This is polled for the
        // whole window, so drive the breathing from here.
        self.refresh();
        match read_presence(self.has_buttons, false) {
            Presence::Grant(level) => level,
            Presence::Pending => consent::Level::None,
        }
    }

    fn set_status(&mut self, status: ui::Status) {
        if status == ui::Status::WaitingForUserPresence {
            begin_button_request();
        }
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
            if up_indicator_wanted() {
                rgb.set(BLUE);
            } else {
                rgb.set(match status {
                    ui::Status::Idle => GREEN,
                    // Processing renders like Idle (green breathe) so boot
                    // activity doesn't flicker teal.
                    ui::Status::Processing => GREEN,
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
            let waiting_for_user = up_indicator_wanted();
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

            #[allow(unused_mut, unused_assignments)]
            let color = if waiting_for_user {
                // breathe fast, in the UP color
                scaled(up, calculate_amplitude(uptime, 2, 4, 75))
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
