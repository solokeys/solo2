//! SoloKeys USB pin definitions + board-specific HAL.
//!
//! Pinout (from the Solo Micro USB schematic):
//!   LED        = P0.03   (single, active-high; PWM-driven)
//!   Cap-touch1 = P0.02   (J1, AIN0)
//!   Cap-touch2 = P0.31   (J2, AIN7)
//!   NFC1       = P0.09   (dedicated NFC pin)
//!   NFC2       = P0.10   (dedicated NFC pin)
//!   SPI to TROPIC01:
//!     SCLK     = P0.13
//!     MOSI/SDO = P0.15   (host out → TROPIC SDI)
//!     MISO/SDI = P0.17   (host in ← TROPIC SDO)
//!     CSn      = P0.14
//!     GPO      = P0.16   (TROPIC notify out → nRF input)
//!     nRESET   = P0.18   (shared with SWD reset)

use crate::cap_touch::CapTouchPad;
use embedded_hal::digital::v2::OutputPin;
use nrf52840_hal::gpio::{p0, Level, Output, Pin, PushPull};
use nrf52840_pac::P0;

/// Single-LED driver for the Solo's P0.03 LED. Active-high: pin HIGH = on.
/// Software-toggled — see board/dk.rs for why we dropped hardware PWM.
pub struct Leds {
    led: Pin<Output<PushPull>>,
}

impl Leds {
    pub fn set_brightness(&mut self, b: u8) {
        let _ = if b >= 128 {
            self.led.set_high() // active-high: HIGH = on
        } else {
            self.led.set_low() // LOW = off
        };
    }
}

pub struct Buttons {
    touch1: CapTouchPad,
    touch2: CapTouchPad,
}

impl Buttons {
    pub fn left(&self) -> bool {
        self.touch1.is_touched()
    }
    pub fn right(&self) -> bool {
        self.touch2.is_touched()
    }
    /// No explicit-deny on Solo — use both-touch.
    pub fn explicit_deny(&self) -> bool {
        false
    }
}

/// Cap-touch pins (P0.02, P0.31) are NOT claimed via the HAL — the driver
/// pokes `PIN_CNF[N]` directly. Calibrates baselines untouched.
pub fn init(p0_periph: P0) -> (Leds, Buttons) {
    let parts = p0::Parts::new(p0_periph);

    let leds = Leds {
        led: parts.p0_03.into_push_pull_output(Level::Low).degrade(),
    };

    let mut touch1 = CapTouchPad::new(2);
    let mut touch2 = CapTouchPad::new(31);
    touch1.calibrate(64);
    touch2.calibrate(64);

    (leds, Buttons { touch1, touch2 })
}
