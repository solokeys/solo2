//! Bounded, hang-proof I2C master for the FM11NT082C NFC frontend.
//!
//! Why not the HAL `I2cMaster`: its blocking Read/Write busy-wait on the master
//! state with NO timeout. A stuck bus (e.g. unpowered adapter, dead pull-ups) then
//! spins forever, the watchdog fires, and the board resets into the ROM bootloader —
//! looking bricked. So this driver drives the I2C register block directly (the enabled
//! `I2cN` `Deref`s to `i2c0::RegisterBlock`) with a bounded spin, and returns an error
//! the moment the bus looks stuck. It implements the embedded-hal 0.2 blocking
//! `Write` + `WriteRead` traits.
//!
//! Board-specific bus (the FM11 sits on a different Flexcomm per board):
//!   - EVK (lpcxpresso55): Flexcomm1, PIO0_13 (SDA) / PIO0_14 (SCL) — true-I2C (Type-I)
//!     pads, so the IOCON EGP bit must be forced to I2C mode (the HAL pin macro leaves
//!     it at reset/GPIO push-pull → the master fights every ACK).
//!   - Solo: Flexcomm4, PIO1_9 (SDA, FC4_CTS_SDA_SSEL0) / PIO1_20 (SCL,
//!     FC4_TXD_SCL_MISO_WS) — normal pads; the HAL typed conversion sets FUNC and we
//!     force open-drain.
//!
//! The pins MUST be configured before the I2C master is enabled, else the master
//! latches a stuck bus.

use crate::hal::{
    self,
    drivers::{clocks::Clocks, pins},
    typestates::init_state::{Enabled, Unknown},
    Iocon, Syscon,
};
use embedded_hal::blocking::i2c;

/// The Flexcomm carrying the FM11 I2C, board-specific.
#[cfg(feature = "lpcxpresso55")]
pub type NfcFlexcomm = hal::peripherals::flexcomm::Flexcomm1<Unknown>;
#[cfg(not(feature = "lpcxpresso55"))]
pub type NfcFlexcomm = hal::peripherals::flexcomm::Flexcomm4<Unknown>;

/// The enabled I2C peripheral, board-specific (both `Deref` to `i2c0::RegisterBlock`).
#[cfg(feature = "lpcxpresso55")]
type NfcI2cPeriph = hal::peripherals::flexcomm::I2c1<Enabled>;
#[cfg(not(feature = "lpcxpresso55"))]
type NfcI2cPeriph = hal::peripherals::flexcomm::I2c4<Enabled>;

/// Bounded spin for a master-pending wait, ~1.5 ms of CPU time — 16x a 100 kHz
/// byte (~90 us), so a healthy transfer never trips it. Kept tight because on a
/// solo board without the 082C the probe bus has no pull-ups: every probe event
/// times out at this bound, and that boot-time cost is paid on every boot
/// (NFC-passive boots are power/deadline constrained).
const SPIN_MAX: u32 = 10_000;

/// Single error type (the frontend requires `Write<Error=E> + WriteRead<Error=E>`).
#[derive(Debug, Copy, Clone)]
pub struct BusError;

pub struct BoundedI2c {
    i2c: NfcI2cPeriph,
}

impl BoundedI2c {
    /// Bring up the board's Flexcomm as a 100 kHz I2C master on the FM11 SDA/SCL pins.
    pub fn setup(
        flexcomm: NfcFlexcomm,
        clocks: &Clocks,
        syscon: &mut Syscon,
        iocon: &mut Iocon<Enabled>,
    ) -> Self {
        let token = clocks.support_flexcomm_token().unwrap();
        let i2c = flexcomm.enabled_as_i2c(syscon, &token);
        let _ = &iocon; // Iocon<Enabled> proves the IOCON clock is on; configured via raw.
        let iocon_raw = unsafe { &*hal::raw::IOCON::ptr() };

        // Configure the pins BEFORE enabling the master (else the master latches a stuck
        // bus while the pads are still GPIO).
        #[cfg(feature = "lpcxpresso55")]
        {
            // EVK true-I2C pads: FUNC1 (FC1 I2C), EGP=I2C, standard slew, 50 ns filter.
            let _sda = pins::Pio0_13::take().unwrap();
            let _scl = pins::Pio0_14::take().unwrap();
            iocon_raw.pio0_13.modify(|_, w| {
                w.func()
                    .alt1()
                    .egp()
                    .i2c_mode()
                    .mode()
                    .inactive()
                    .digimode()
                    .digital()
                    .slew()
                    .standard()
                    .i2cfilter()
                    .fast_mode()
            });
            iocon_raw.pio0_14.modify(|_, w| {
                w.func()
                    .alt1()
                    .egp()
                    .i2c_mode()
                    .mode()
                    .inactive()
                    .digimode()
                    .digital()
                    .slew()
                    .standard()
                    .i2cfilter()
                    .fast_mode()
            });
        }
        #[cfg(not(feature = "lpcxpresso55"))]
        {
            // Solo FC4 on normal pads: HAL typed conversion sets FUNC, then
            // force digital + open-drain.
            let _sda = pins::Pio1_9::take().unwrap().into_i2c4_sda_pin(iocon);
            let _scl = pins::Pio1_20::take().unwrap().into_i2c4_scl_pin(iocon);
            iocon_raw
                .pio1_9
                .modify(|_, w| w.digimode().digital().od().open_drain());
            iocon_raw
                .pio1_20
                .modify(|_, w| w.digimode().digital().od().open_drain());
        }

        // 100 kHz off the 12 MHz Flexcomm clock: clkdiv holds div-1; msttime hold scl-2.
        i2c.cfg.modify(|_, w| w.msten().enabled());
        i2c.clkdiv.modify(|_, w| unsafe { w.divval().bits(9) });
        i2c.msttime
            .modify(|_, w| w.mstsclhigh().bits(4).mstscllow().bits(4));

        Self { i2c }
    }

    /// Raw address probe (START + addr(W) + STOP), no driver, no pins consumed. True
    /// if a device ACKs the address — used to decide 082C-vs-NC08 before taking the
    /// IRQ pin. Bounded; best-effort STOP on error.
    pub fn probe(&mut self, addr: u8) -> bool {
        let r = self.start(addr << 1).and_then(|_| self.stop());
        if r.is_err() {
            self.recover();
        }
        r.is_ok()
    }

    /// Bounded wait for the master to become non-pending. Err(()) on timeout.
    fn wait_pending(&self) -> Result<(), BusError> {
        let mut n = 0u32;
        while self.i2c.stat.read().mstpending().is_in_progress() {
            n += 1;
            if n >= SPIN_MAX {
                return Err(BusError);
            }
        }
        Ok(())
    }

    /// Error if the master state shows a NACK / arbitration loss / start-stop error.
    fn check(&self) -> Result<(), BusError> {
        let stat = self.i2c.stat.read();
        if stat.mststate().is_nack_address()
            || stat.mststate().is_nack_data()
            || stat.mstarbloss().is_arbitration_loss()
            || stat.mstststperr().is_error()
        {
            Err(BusError)
        } else {
            Ok(())
        }
    }

    /// Best-effort STOP so one failed transfer doesn't wedge the master.
    fn recover(&self) {
        self.i2c.mstctl.write(|w| w.mststop().stop());
        let _ = self.wait_pending();
    }

    fn start(&self, byte: u8) -> Result<(), BusError> {
        self.i2c
            .mstdat
            .modify(|_, w| unsafe { w.data().bits(byte) });
        self.i2c.mstctl.write(|w| w.mststart().start());
        self.wait_pending()?;
        self.check()
    }

    fn cont(&self, byte: u8) -> Result<(), BusError> {
        self.i2c
            .mstdat
            .modify(|_, w| unsafe { w.data().bits(byte) });
        self.i2c.mstctl.write(|w| w.mstcontinue().continue_());
        self.wait_pending()?;
        self.check()
    }

    fn stop(&self) -> Result<(), BusError> {
        self.i2c.mstctl.write(|w| w.mststop().stop());
        self.wait_pending()
    }
}

impl i2c::Write for BoundedI2c {
    type Error = BusError;

    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), BusError> {
        let r = (|| {
            self.start(addr << 1)?;
            for &b in bytes {
                self.cont(b)?;
            }
            self.stop()
        })();
        if r.is_err() {
            self.recover();
        }
        r
    }
}

impl i2c::WriteRead for BoundedI2c {
    type Error = BusError;

    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), BusError> {
        let r = (|| {
            // Phase 1: write the target (memory) address, no STOP.
            self.start(addr << 1)?;
            for &b in bytes {
                self.cont(b)?;
            }
            // Phase 2: repeated START + addr(read), clock the bytes out.
            self.start((addr << 1) | 1)?;
            let n = buffer.len();
            for (i, slot) in buffer.iter_mut().enumerate() {
                if !self.i2c.stat.read().mststate().is_receive_ready() {
                    return Err(BusError);
                }
                *slot = self.i2c.mstdat.read().data().bits();
                if i + 1 < n {
                    self.i2c.mstctl.write(|w| w.mstcontinue().continue_());
                    self.wait_pending()?;
                    self.check()?;
                }
            }
            self.stop()
        })();
        if r.is_err() {
            self.recover();
        }
        r
    }
}
