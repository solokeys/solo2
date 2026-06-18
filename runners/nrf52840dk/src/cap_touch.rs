//! Polled capacitive-touch driver, board-agnostic.
//!
//! Hardware: any GPIO with the relaxation-oscillator circuit
//!   GPIO ── 20pF series cap ── pad ; pad-to-GND via 1MΩ.
//! Used on the SoloKeys USB board (built-in pads on P0.02 + P0.31)
//! and on the nRF52840-DK (manually wired pads on P0.03 + P0.04 — see
//! board/dk.rs for the wiring diagram).
//!
//! Algorithm (per scan):
//!   1. Configure pin as output, drive HIGH → charges the pad cap.
//!   2. Wait long enough to saturate (~25 µs at 64 MHz).
//!   3. Switch pin to input (no pull) → cap discharges through 1 MΩ.
//!   4. Spin-loop counting iterations until the input reads LOW.
//!   5. Touch increases pad capacitance (finger ~5–20 pF) → longer
//!      discharge time → higher loop count.
//!
//! Calibration: take a baseline at boot (untouched). At runtime, a
//! reading > `baseline * threshold_pct / 100` counts as touched.
//!
//! CPU cost: ~50–100 µs per scan. Run at ~50 ms cadence → ~0.2 % CPU
//! per pad, negligible. No peripheral allocated — direct PIN_CNF pokes
//! on the P0 base register are portable across all nRF52840 boards.
//!
//! All pads on the nRF52840 P0 register block are supported. P1.x pads
//! would need a sister struct pointing at P1::ptr().

use nrf52840_pac::P0;

pub struct CapTouchPad {
    pin: u8,            // GPIO bit position 0..32 within P0
    baseline: u32,      // calibrated discharge count when untouched
    threshold_pct: u32, // touched if reading > baseline * pct / 100
}

const DEFAULT_THRESHOLD_PCT: u32 = 130;
const CHARGE_CYCLES: u32 = 2_000; // ~31 µs at 64 MHz
const MAX_LOOP_COUNT: u32 = 50_000; // hard ceiling per scan (~ms-range)

impl CapTouchPad {
    /// Construct a pad driver bound to a P0 GPIO bit. The caller must
    /// guarantee the pin isn't otherwise driven by the HAL — this driver
    /// reconfigures `PIN_CNF[N]` on every scan.
    pub fn new(pin: u8) -> Self {
        debug_assert!(pin < 32);
        Self {
            pin,
            baseline: 0,
            threshold_pct: DEFAULT_THRESHOLD_PCT,
        }
    }

    /// Take `samples` measurements and use the average as the
    /// untouched baseline. Call once at boot, untouched.
    pub fn calibrate(&mut self, samples: u32) {
        debug_assert!(samples > 0);
        let mut sum = 0u64;
        for _ in 0..samples {
            sum += self.measure() as u64;
        }
        self.baseline = (sum / samples as u64) as u32;
    }

    /// Override the touch threshold (default 130 = 30 % above baseline).
    /// Lower → more sensitive (more false positives); higher → less
    /// sensitive (might miss light touches).
    #[allow(dead_code)]
    pub fn set_threshold_pct(&mut self, pct: u32) {
        self.threshold_pct = pct;
    }

    #[allow(dead_code)]
    pub fn baseline(&self) -> u32 {
        self.baseline
    }

    /// `true` if the most recent measurement exceeds the touch threshold.
    pub fn is_touched(&self) -> bool {
        self.measure() > self.baseline.saturating_mul(self.threshold_pct) / 100
    }

    /// Raw discharge-count measurement. Useful for tuning + diagnostics.
    pub fn measure(&self) -> u32 {
        let p0 = unsafe { &*P0::ptr() };
        let bit = self.pin as usize;
        let mask = 1u32 << self.pin;

        // Charge: drive output HIGH.
        p0.pin_cnf[bit].write(|w| {
            w.dir().output();
            w.input().disconnect();
            w.pull().disabled();
            w.drive().s0s1();
            w.sense().disabled();
            w
        });
        p0.outset.write(|w| unsafe { w.bits(mask) });
        cortex_m::asm::delay(CHARGE_CYCLES);

        // Switch to input, no pull.
        p0.pin_cnf[bit].write(|w| {
            w.dir().input();
            w.input().connect();
            w.pull().disabled();
            w.drive().s0s1();
            w.sense().disabled();
            w
        });

        // Spin until LOW; cap discharges through the external 1 MΩ.
        let in_reg = &p0.in_;
        let mut count = 0u32;
        while count < MAX_LOOP_COUNT {
            if in_reg.read().bits() & mask == 0 {
                break;
            }
            count = count.wrapping_add(1);
        }
        count
    }
}
