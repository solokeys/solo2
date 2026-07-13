#![no_std]
//! FM11NT082C — Fudan NFC dual-interface tag + channel IC, driven over **I2C** in
//! **NC (channel) mode** for ISO14443-4 / FIDO-over-NFC.
//!
//! This is the I2C sibling of the `fm11nc08` SPI driver. The 082C exposes the *same*
//! register architecture as the FM11NC08 — the `MAIN_IRQ` and `FIFO_IRQ` bit layouts
//! are bit-for-bit identical — just at I2C memory registers `0xFFF0..=0xFFFC` (2-byte
//! addressing) instead of SPI register indices. So `read_packet`/`send_packet` mirror
//! the FM11NC08 logic almost verbatim; only the register access (SPI cmd -> I2C 2-byte
//! addressed read/write) and the config path (EEPROM config word over I2C, no Fudan
//! auth on the contact side) differ.
//!
//! HW-VERIFY markers below flag the few things that must be confirmed on hardware:
//!   - FIFO burst POP semantics (a multi-byte read at 0xFFF0 must pop successive FIFO
//!     bytes, NOT auto-increment into 0xFFF1/2). Datasheet 3.3.3.9 + the read example
//!     (read WORDCNT, then read N FIFO bytes) imply POP; confirm on the EVK.
//!   - the exact USER_CFG bits / CHK for NC + ISO14443-4, and whether the config-word
//!     write needs the chip power-cycled to take effect.

use embedded_hal as hal;
use embedded_time::duration::{Extensions, Microseconds};
use hal::{blocking::i2c, digital::v2::InputPin, timer::CountDown};

use nfc_device::traits::nfc;

/// Fudan factory-default 7-bit I2C slave address (confirmed on our sample).
pub const DEFAULT_ADDR: u8 = 0x57;

/// NC-mode registers (full 16-bit I2C memory address; access is 2-byte-addressed).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Register {
    FifoAccess = 0xFFF0,
    FifoClear = 0xFFF1,
    FifoWordCnt = 0xFFF2,
    NfcStatus = 0xFFF3,
    NfcTxEn = 0xFFF4,
    NfcCfg = 0xFFF5,
    NfcRats = 0xFFF6,
    MainIrq = 0xFFF7,
    FifoIrq = 0xFFF8,
    AuxIrq = 0xFFF9,
    MainIrqMask = 0xFFFA,
    FifoIrqMask = 0xFFFB,
    AuxIrqMask = 0xFFFC,
}

/// MAIN_IRQ bits — identical layout to the FM11NC08 `Interrupt`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Interrupt {
    Aux = 1 << 0,
    Fifo = 1 << 1,
    Arbitration = 1 << 2,
    TxDone = 1 << 3,
    RxDone = 1 << 4,
    RxStart = 1 << 5,
    Active = 1 << 6,
    RfPower = 1 << 7,
}

/// FIFO_IRQ bits — identical layout to the FM11NC08 `FifoInterrupt`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FifoInterrupt {
    Empty = 1 << 0,
    Full = 1 << 1,
    OverFlow = 1 << 2,
    WaterLevel = 1 << 3,
}

/// EEPROM config layout (2-byte addressed). Contact-side writes need no Fudan auth
/// (datasheet 3.5.1: the security algorithm gates the *contactless* interface only).
mod eeprom {
    /// Config word block E4: [USER_CFG0, USER_CFG1, USER_CFG2, CHK]; CHK valid iff
    /// CHK == !(CFG0 ^ CFG1 ^ CFG2). USER_CFG0 bit0 (OP_MODE_SELECT): 0=NT, 1=NC.
    pub const USER_CFG: u16 = 0x0390;
    /// ATS TL/T0 (+ VOUT_RES_CFG / I2C addr in the same block).
    pub const ATS_TL_T0: u16 = 0x03B0;
    /// ATS TA/TB/TC.
    pub const ATS_TA_TB_TC: u16 = 0x03B4;
    /// ATQA (2 bytes).
    pub const ATQA: u16 = 0x03BC;
    /// SAK1, SAK2.
    pub const SAK: u16 = 0x03BE;
}

/// ISO14443-4 activation parameters — the same values the proven FM11NC08 setup uses
/// (board/src/nfc.rs): ATQA 0x0044, SAK T4T 0x20 (bit5 = ISO14443-4 compliant).
pub struct Configuration {
    /// USER_CFG0: OP_MODE_SELECT=1 (NC) and NFC_mode[3:2]=00 (ISO14443-4).
    pub user_cfg0: u8,
    pub user_cfg1: u8,
    pub user_cfg2: u8,
    pub atqa: u16,
    pub sak1: u8,
    pub sak2: u8,
    pub tl: u8,
    pub t0: u8,
    pub ta: u8,
    pub tb: u8,
    pub tc: u8,
}

fn fsdi_to_frame_size(fsdi: u8) -> usize {
    match fsdi {
        0 => 16,
        1 => 24,
        2 => 32,
        3 => 40,
        4 => 48,
        5 => 64,
        6 => 96,
        7 => 128,
        _ => 256,
    }
}

pub struct FM11NT082C<I2C, INT> {
    i2c: I2C,
    addr: u8,
    pub int: INT,
    packet: [u8; 256],
    offset: usize,
    current_frame_size: usize,
    /// True between RxStart and RxDone. Gates the water-level FIFO drain: the
    /// TX FIFO latches Empty|WaterLevel after a transmission, and draining on
    /// those stale flags would pop the first byte(s) of the next inbound frame.
    mid_rx: bool,
}

impl<I2C, INT, E> FM11NT082C<I2C, INT>
where
    I2C: i2c::Write<Error = E> + i2c::WriteRead<Error = E>,
    INT: InputPin,
{
    pub fn new(i2c: I2C, addr: u8, int: INT) -> Self {
        Self {
            i2c,
            addr,
            int,
            packet: [0u8; 256],
            offset: 0,
            current_frame_size: 128,
            mid_rx: false,
        }
    }

    pub fn enabled(self) -> Self {
        self
    }

    fn addr_bytes(reg: u16) -> [u8; 2] {
        [(reg >> 8) as u8, reg as u8]
    }

    /// Read one register (2-byte addressed). Errors surface as 0xFF (matches "no chip").
    pub fn read_reg(&mut self, reg: Register) -> u8 {
        self.read_mem_u8(reg as u16)
    }

    fn read_mem_u8(&mut self, reg: u16) -> u8 {
        let mut buf = [0u8; 1];
        match self
            .i2c
            .write_read(self.addr, &Self::addr_bytes(reg), &mut buf)
        {
            Ok(()) => buf[0],
            Err(_) => 0xFF,
        }
    }

    pub fn write_reg(&mut self, reg: Register, data: u8) {
        self.write_mem(reg as u16, &[data]);
    }

    /// Write `data` at a 2-byte memory address (registers or EEPROM). Best-effort.
    fn write_mem(&mut self, reg: u16, data: &[u8]) {
        // 2 address bytes + up to a 32-byte FIFO / config chunk.
        let mut scratch = [0u8; 2 + 32];
        let n = data.len().min(32);
        let ab = Self::addr_bytes(reg);
        scratch[0] = ab[0];
        scratch[1] = ab[1];
        scratch[2..2 + n].copy_from_slice(&data[..n]);
        self.i2c.write(self.addr, &scratch[..2 + n]).ok();
    }

    fn read_mem(&mut self, reg: u16, buf: &mut [u8]) -> bool {
        self.i2c
            .write_read(self.addr, &Self::addr_bytes(reg), buf)
            .is_ok()
    }

    /// Presence check used for runtime detection: read the chip serial (EEPROM 0x0000);
    /// SN0 == 0x1D is Fudan's manufacturer code. Returns false if nothing ACKs.
    pub fn is_present(&mut self) -> bool {
        let mut sn = [0u8; 1];
        self.read_mem(0x0000, &mut sn) && sn[0] == 0x1D
    }

    /// Read `buf.len()` bytes from a 2-byte memory address (EEPROM/registers). For
    /// bring-up verification over SWD. Returns false on bus error.
    pub fn read_bytes(&mut self, addr: u16, buf: &mut [u8]) -> bool {
        self.read_mem(addr, buf)
    }

    /// One-time EEPROM configuration: put the chip in NC mode + ISO14443-4 with the
    /// SoloKeys ATQA/SAK/ATS. Contact-side (I2C) writes are unauthenticated. Each
    /// EEPROM write needs ~10 ms (tWR). Effective after the chip is next powered on.
    #[allow(clippy::result_unit_err)]
    pub fn configure(
        &mut self,
        config: Configuration,
        timer: &mut impl CountDown<Time = Microseconds>,
    ) -> Result<(), ()> {
        let chk = !(config.user_cfg0 ^ config.user_cfg1 ^ config.user_cfg2);
        // HW-VERIFY: config-word write over I2C + power-cycle to switch NT->NC.
        self.write_eeprom(
            eeprom::USER_CFG,
            &[config.user_cfg0, config.user_cfg1, config.user_cfg2, chk],
            timer,
        );
        self.write_eeprom(eeprom::ATQA, &config.atqa.to_be_bytes(), timer);
        self.write_eeprom(eeprom::SAK, &[config.sak1, config.sak2], timer);
        self.write_eeprom(eeprom::ATS_TL_T0, &[config.tl, config.t0], timer);
        self.write_eeprom(
            eeprom::ATS_TA_TB_TC,
            &[config.ta, config.tb, config.tc],
            timer,
        );

        // Apply the freshly-written EEPROM config by soft-resetting: the chip re-reads
        // EEPROM into its live registers (NC mode, ISO14443-4, SAK=0x20, arbitration) —
        // no physical power-cycle. The anticollision SAK is EEPROM-only, so this is what
        // actually makes a reader see ISO14443-4. Then ensure the RF interface is active.
        self.write_mem(0xFFE6, &[0x55]); // RESET_SILENCE.soft_reset
        timer.start(5_000.microseconds());
        block(timer);
        self.write_mem(0xFFE6, &[0xCC]); // RESET_SILENCE.NFC_silence -> active
        Ok(())
    }

    fn write_eeprom(
        &mut self,
        addr: u16,
        data: &[u8],
        timer: &mut impl CountDown<Time = Microseconds>,
    ) {
        self.write_mem(addr, data);
        timer.start(10_000.microseconds());
        block(timer);
    }

    /// After configure()+power-on, arm the interrupts: fire ONLY on RxDone/TxDone/Fifo,
    /// NOT Active/RxStart. Reading MAIN_IRQ mid-reception resets the contactless RX via
    /// contact/contactless arbitration (082C is contact-first), so firing on Active/
    /// RxStart would make us poll *during* RX and self-reset. Ported from FM11NC08.
    pub fn arm_interrupts(&mut self) {
        self.write_reg(Register::AuxIrqMask, 0x00);
        self.write_reg(
            Register::FifoIrqMask,
            0xff ^ (FifoInterrupt::WaterLevel as u8) ^ (FifoInterrupt::Full as u8),
        );
        self.write_reg(
            Register::MainIrqMask,
            0xff ^ (Interrupt::RxDone as u8) ^ (Interrupt::TxDone as u8) ^ (Interrupt::Fifo as u8),
        );
    }

    /// Switch to NC (channel) mode + ISO14443-4 **immediately** via the live USER_CFG
    /// registers (0xFFE0/0xFFE1), so no power-cycle is needed. This is temporary (resets
    /// on power-off); `configure()`'s EEPROM write makes it persist across power cycles.
    /// Contact-side (I2C) — no Fudan auth. Returns (USER_CFG0, USER_CFG1) as read back.
    pub fn enter_nc_iso14443_4(&mut self) -> (u8, u8) {
        let cfg0 = self.read_mem_u8(0xFFE0) | 0x01; // OP_MODE_SELECT = 1 (NC)
        self.write_mem(0xFFE0, &[cfg0]);
        let cfg1 = self.read_mem_u8(0xFFE1) & !0x0C; // NFC_mode[3:2] = 00 (ISO14443-4)
        self.write_mem(0xFFE1, &[cfg1]);
        // arbitration_cfg (USER_CFG2 bits[5:4]): 10=contact-first STARVES the RF under
        // our constant polling. 01=first-come-first-served lets an in-progress RF
        // transaction complete, so the reader can be seen.
        let cfg2 = (self.read_mem_u8(0xFFE2) & !0x30) | 0x10;
        self.write_mem(0xFFE2, &[cfg2]);
        (self.read_mem_u8(0xFFE0), self.read_mem_u8(0xFFE1))
    }

    /// Soft-reset the chip (RESET_SILENCE.soft_reset magic 0x55) so it re-reads the
    /// EEPROM config into its live registers — applies a fresh `configure()` (NC mode,
    /// ISO14443-4, SAK=0x20, arbitration) WITHOUT a physical power-cycle. The anticollision
    /// SAK is EEPROM-only, so this (not the live USER_CFG register writes) is what makes
    /// the reader see ISO14443-4. Caller must delay ~5 ms afterwards.
    pub fn soft_reset(&mut self) {
        self.write_mem(0xFFE6, &[0x55]);
    }

    /// Force the contactless (RF) interface into the active (non-silent) state
    /// (RESET_SILENCE.NFC_silence magic 0xCC).
    pub fn contactless_active(&mut self) {
        self.write_mem(0xFFE6, &[0xCC]);
    }

    /// Read the live USER_CFG0/1/2 registers (SWD verification).
    pub fn user_cfg(&mut self) -> (u8, u8, u8) {
        (
            self.read_mem_u8(0xFFE0),
            self.read_mem_u8(0xFFE1),
            self.read_mem_u8(0xFFE2),
        )
    }

    pub fn has_interrupt(&mut self) -> nb::Result<(), nfc::Error> {
        if self.int.is_low().ok().unwrap_or(false) {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// POP `count` bytes from the FIFO into `self.packet[self.offset..]`.
    /// HW-VERIFY: assumes a burst read at 0xFFF0 pops successive bytes (no auto-inc).
    fn read_fifo(&mut self, count: u8) {
        let n = count as usize;
        let start = self.offset;
        let end = (start + n).min(self.packet.len());
        let ab = Self::addr_bytes(Register::FifoAccess as u16);
        self.i2c
            .write_read(self.addr, &ab, &mut self.packet[start..end])
            .ok();
    }

    /// PUSH `buf` (<=32 bytes) to the FIFO.
    fn write_fifo(&mut self, buf: &[u8]) {
        if buf.is_empty() {
            return;
        }
        self.write_mem(Register::FifoAccess as u16, buf);
    }

    pub fn read_packet(&mut self, buf: &mut [u8]) -> Result<nfc::State, nfc::Error> {
        let main_irq = self.read_reg(Register::MainIrq);
        let mut new_session = false;

        let fifo_irq = if (main_irq & Interrupt::Fifo as u8) != 0 {
            self.read_reg(Register::FifoIrq)
        } else {
            0
        };

        // AUX_IRQ (RF error bits) is clear-on-read; read it or its summary bit
        // stays latched in MAIN_IRQ.
        if (main_irq & Interrupt::Aux as u8) != 0 {
            let _ = self.read_reg(Register::AuxIrq);
        }

        if main_irq & (Interrupt::Active as u8) != 0 {
            self.offset = 0;
            self.mid_rx = false;
            new_session = true;
        }

        if main_irq & (Interrupt::RxStart as u8) != 0 {
            self.offset = 0;
            self.mid_rx = true;
            let rats = self.read_reg(Register::NfcRats);
            self.current_frame_size = fsdi_to_frame_size((rats >> 4) & 0xf);
        }

        if main_irq & (Interrupt::RxDone as u8) != 0 {
            self.mid_rx = false;
            let count = self.read_reg(Register::FifoWordCnt);
            if count > 0 && count < 32 {
                self.read_fifo(count);
                self.offset += count as usize;
            }
            if self.offset <= 2 {
                self.offset = 0;
            } else {
                // Drop the 2 trailing CRC bytes.
                let l = self.offset - 2;
                buf[..l].copy_from_slice(&self.packet[..l]);
                self.offset = 0;
                return if new_session {
                    Ok(nfc::State::NewSession(l as u8))
                } else {
                    Ok(nfc::State::Continue(l as u8))
                };
            }
        }

        // Drain at the water level while a frame is still arriving (`mid_rx`,
        // see field docs).
        if self.mid_rx && (fifo_irq & (FifoInterrupt::WaterLevel as u8) != 0) {
            let nfc_status = self.read_reg(Register::NfcStatus);
            if (nfc_status & 1) == 0 {
                let count = self.read_reg(Register::FifoWordCnt);
                self.read_fifo(count);
                self.offset += count as usize;
            }
        }

        if new_session {
            Err(nfc::Error::NewSession)
        } else {
            Err(nfc::Error::NoActivity)
        }
    }

    fn wait_for_transmission(&mut self) -> Result<(), ()> {
        self.write_reg(Register::NfcTxEn, 0x55);
        // Wait until the FIFO drains below the chip's 8-byte TX water level —
        // room for the next 24-byte chunk (the FIFO is 32 deep).
        for _ in 0..40 {
            if self.read_reg(Register::FifoWordCnt) < 8 {
                return Ok(());
            }
        }
        Err(())
    }

    pub fn send_packet(&mut self, buf: &[u8]) -> Result<(), nfc::Error> {
        // Write in <=24-byte chunks, waiting for the water level between chunks.
        for i in 0..buf.len() / 24 {
            self.write_fifo(&buf[i * 24..i * 24 + 24]);
            if self.wait_for_transmission().is_err() {
                return Err(nfc::Error::NoActivity);
            }
        }
        self.write_fifo(&buf[(buf.len() / 24) * 24..]);
        self.wait_for_transmission().ok();
        Ok(())
    }

    pub fn release(self) -> (I2C, INT) {
        (self.i2c, self.int)
    }
}

/// Bounded blocking wait on a CountDown (embedded-hal 0.2 `nb`).
fn block(timer: &mut impl CountDown) {
    while timer.wait().is_err() {}
}

impl<I2C, INT, E> nfc::Device for FM11NT082C<I2C, INT>
where
    I2C: i2c::Write<Error = E> + i2c::WriteRead<Error = E>,
    INT: InputPin,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<nfc::State, nfc::Error> {
        self.read_packet(buf)
    }
    fn send(&mut self, buf: &[u8]) -> Result<(), nfc::Error> {
        self.send_packet(buf)
    }
    fn frame_size(&self) -> usize {
        self.current_frame_size
    }
}
