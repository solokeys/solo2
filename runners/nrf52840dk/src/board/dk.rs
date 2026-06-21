//! nRF52840-DK pin definitions + board-specific HAL.
//!
//! Pinout:
//!   LED4=P0.16  (active-low, PWM-driven; LED1-3 are physically there but
//!                left parked off in this firmware — only LED4 carries
//!                state. Same single-LED model as the SoloKeys.)
//!   SW1=P0.11   SW2=P0.12   SW3=P0.24   SW4=P0.25   (active-low pull-up;
//!                Btn4 unused, the bootloader's button-DFU trigger uses it)
//!   Cap-touch1=P0.03  Cap-touch2=P0.04
//!
//! ── Cap-touch wiring (manual, on the DK's analog header P3) ──────────
//!   P0.03 (A0) ── 20pF ──┬── [touch pad: bare wire / copper tape / foil]
//!                        └── 1 MΩ ── GND
//!   P0.04 (A1) ── 20pF ──┬── [touch pad]
//!                        └── 1 MΩ ── GND

use crate::cap_touch::CapTouchPad;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use nrf52840_hal::gpio::{p0, Input, Level, Output, Pin, PullUp, PushPull};
use nrf52840_pac::P0;

/// LEDs on the DK.
/// Active-low: pin LOW = LED on. led4 is software-toggled by trussed
/// status; led3 is held low forever as a "firmware alive" heartbeat.
/// (Owning the pin handle in the Leds struct prevents the compiler
/// from optimising the init away — `let _ = …` had this exact issue.)
pub struct Leds {
    led4: Pin<Output<PushPull>>,
    _led3: Pin<Output<PushPull>>,
}

impl Leds {
    pub fn set_brightness(&mut self, b: u8) {
        let _ = if b >= 128 {
            self.led4.set_low() // active-low: LOW = on
        } else {
            self.led4.set_high() // HIGH = off
        };
    }
}

pub struct Buttons {
    pub btn1: Pin<Input<PullUp>>,
    pub btn2: Pin<Input<PullUp>>,
    pub btn3: Pin<Input<PullUp>>,
    pub touch1: CapTouchPad,
    pub touch2: CapTouchPad,
}

impl Buttons {
    /// "Left" approve source: Btn1 (P0.11) OR cap-touch1 (P0.03).
    pub fn left(&self) -> bool {
        self.btn1.is_low().unwrap_or(false) || self.touch1.is_touched()
    }
    /// "Right" approve source: Btn2 (P0.12) OR cap-touch2 (P0.04).
    pub fn right(&self) -> bool {
        self.btn2.is_low().unwrap_or(false) || self.touch2.is_touched()
    }
    /// Explicit deny shortcut: Btn3 (P0.24).
    pub fn explicit_deny(&self) -> bool {
        self.btn3.is_low().unwrap_or(false)
    }
}

/// Take ownership of the P0 GPIO bank, configure pins, calibrate cap-touch
/// baselines (untouched). Returns Leds + Buttons.
///
/// Cap-touch pins (P0.03, P0.04) are NOT claimed via the HAL — the driver
/// pokes `PIN_CNF[N]` directly on every measurement. Don't also `into_*`
/// them here or the HAL type-state will fight the driver.
pub fn init(p0_periph: P0) -> (Leds, Buttons) {
    let parts = p0::Parts::new(p0_periph);

    // LED1, LED2 parked off (drop their handles — pins stay HIGH = off).
    let _ = parts.p0_13.into_push_pull_output(Level::High).degrade();
    let _ = parts.p0_14.into_push_pull_output(Level::High).degrade();

    let mut leds = Leds {
        // LED3 owned but never touched again — pin held LOW = always on.
        _led3: parts.p0_15.into_push_pull_output(Level::Low).degrade(),
        led4: parts.p0_16.into_push_pull_output(Level::High).degrade(),
    };

    // Boot indication: 3 quick LED4 blinks (sync). Moved here from
    // `UserInterface::new` when the LEDs were hoisted to the idle loop.
    const BLINK_CYCLES: u32 = 64_000_000 / 5 / 4; // ~200 ms at 64 MHz
    for _ in 0..3 {
        leds.set_brightness(255);
        cortex_m::asm::delay(BLINK_CYCLES);
        leds.set_brightness(0);
        cortex_m::asm::delay(BLINK_CYCLES);
    }

    let mut touch1 = CapTouchPad::new(3);
    let mut touch2 = CapTouchPad::new(4);
    // Calibrate untouched. 64 samples × ~50 µs ≈ 3 ms — boot delay imperceptible.
    touch1.calibrate(64);
    touch2.calibrate(64);

    let buttons = Buttons {
        btn1: parts.p0_11.into_pullup_input().degrade(),
        btn2: parts.p0_12.into_pullup_input().degrade(),
        btn3: parts.p0_24.into_pullup_input().degrade(),
        touch1,
        touch2,
    };

    (leds, buttons)
}
