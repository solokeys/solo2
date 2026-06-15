//! Shared Spi0 (FlexComm0) bus for the FM11NC08 NFC reader and the external
//! SPI-NOR flash. Both chips sit on the same SCK/MOSI/MISO (PIO0_28/24/25)
//! with independent GPIO chip-selects (NFC = PIO1_20, flash = PIO0_13).
//!
//! Common 2 MHz at fixed polarity; only CPHA toggles per device, since the
//! flash needs SPI mode 0 (CPHA=0) and the FM11NC08 needs mode 1 (CPHA=1).
//! [`LockCs`] sets it on chip-select (see `set_phase`).
//!
//! Mutual exclusion: a transaction is `CS low -> bytes -> CS high`. The
//! chip-select wrapper [`LockCs`] acquires a global critical section when it
//! drives CS low and releases it when CS goes high, so a whole transaction is
//! atomic against the priority-7 NFC IRQ (and vice-versa). The flash `Storage`
//! impl is reached deep in the trussed call stack with no RTIC lock token in
//! scope, which is why this is a critical section rather than an RTIC resource.

use core::cell::UnsafeCell;

use crate::hal::{
    self,
    drivers::{pins, Pin, SpiMaster},
    peripherals::flexcomm::Spi0,
    time::RateExtensions,
    typestates::pin::{self, flexcomm::NoPio},
    Enabled, Iocon,
};
use embedded_hal::digital::v2::OutputPin;
use hal::traits::wg::spi::{FullDuplex, Mode, Phase, Polarity};

// Shared bus pins (identical in nfc.rs and flash.rs).
pub type SckPin = pins::Pio0_28;
pub type MosiPin = pins::Pio0_24;
pub type MisoPin = pins::Pio0_25;

type Sck = Pin<SckPin, pin::state::Special<pin::function::FC0_SCK>>;
type Mosi = Pin<MosiPin, pin::state::Special<pin::function::FC0_RXD_SDA_MOSI_DATA>>;
type Miso = Pin<MisoPin, pin::state::Special<pin::function::FC0_TXD_SCL_MISO_WS>>;

/// The single SPI master that owns the bus. Built once at 2 MHz; CPHA is
/// toggled per device via `set_phase`.
pub type SharedSpiMaster =
    SpiMaster<SckPin, MosiPin, MisoPin, NoPio, Spi0, (Sck, Mosi, Miso, pin::flexcomm::NoCs)>;

type BusError = <SharedSpiMaster as FullDuplex<u8>>::Error;

/// `UnsafeCell` is not `Sync`; access is only ever performed while a
/// [`LockCs`] holds the global critical section (interrupts off), so there is
/// no concurrent access.
struct BusCell(UnsafeCell<Option<SharedSpiMaster>>);
unsafe impl Sync for BusCell {}

static BUS: BusCell = BusCell(UnsafeCell::new(None));

/// Current bus CPHA, so we only reprogram CFG on an actual device switch.
/// Starts `false` (mode 0) to match the config `setup` builds.
static CURRENT_SECOND: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Reprogram the bus phase (CPHA) for the device about to transact. The LPC55
/// requires the SPI peripheral disabled while CFG changes, so disable -> set
/// cpha -> enable. Called under the bus critical section (interrupts off), so
/// the read of `CURRENT_SECOND` and the register sequence are atomic.
fn set_phase(second_transition: bool) {
    use core::sync::atomic::Ordering;
    if CURRENT_SECOND.load(Ordering::Relaxed) == second_transition {
        return;
    }
    // SAFETY: exclusive access to the FlexComm0 SPI registers under the
    // critical section; only CFG.enable / CFG.cpha are touched.
    let spi = unsafe { &*hal::raw::SPI0::ptr() };
    spi.cfg.modify(|_, w| w.enable().disabled());
    spi.cfg.modify(|_, w| w.cpha().bit(second_transition));
    spi.cfg.modify(|_, w| w.enable().enabled());
    CURRENT_SECOND.store(second_transition, Ordering::Relaxed);
}

/// Install the bus master. Call once during init before any transaction.
pub fn install(master: SharedSpiMaster) {
    critical_section::with(|_| unsafe {
        *BUS.0.get() = Some(master);
    });
}

/// Build the shared bus from `Spi0` + the SCK/MOSI/MISO pins and install it.
/// Call once during init, before NFC or flash bring-up. Starts in mode 0
/// (CPOL=0/CPHA=0); `LockCs`/`set_phase` toggle CPHA per device after that.
pub fn setup(spi: Spi0<Enabled>, iocon: &mut Iocon<Enabled>) {
    let sck = SckPin::take().unwrap().into_spi0_sck_pin(iocon);
    let mosi = MosiPin::take().unwrap().into_spi0_mosi_pin(iocon);
    let miso = MisoPin::take().unwrap().into_spi0_miso_pin(iocon);
    let mode = Mode {
        polarity: Polarity::IdleLow,
        phase: Phase::CaptureOnFirstTransition,
    };
    let master = SpiMaster::new(
        spi,
        (sck, mosi, miso, pin::flexcomm::NoCs),
        2_000_000u32.Hz(),
        mode,
    );
    install(master);
}

/// Chip-select wrapper that makes a transaction atomic on the shared bus.
///
/// Driving CS low acquires the global critical section; driving CS high
/// releases it. The wrapped driver (FM11NC08 / spi_memory) keeps CS low for
/// the whole `cmd + data` exchange, so the critical section spans exactly one
/// bus transaction.
pub struct LockCs<P>
where
    P: OutputPin,
{
    pin: P,
    second_transition: bool,
    restore: Option<critical_section::RestoreState>,
}

impl<P> LockCs<P>
where
    P: OutputPin,
{
    /// `second_transition` picks this device's bus phase: `true` = SPI mode 1
    /// (CPHA=1, FM11NC08), `false` = mode 0 (CPHA=0, SPI-NOR flash). Speed is
    /// common (2 MHz), so only CPHA is toggled per device.
    pub fn new(pin: P, second_transition: bool) -> Self {
        Self {
            pin,
            second_transition,
            restore: None,
        }
    }
}

impl<P> OutputPin for LockCs<P>
where
    P: OutputPin,
{
    type Error = P::Error;

    fn set_low(&mut self) -> Result<(), Self::Error> {
        // Acquire first so the phase change + assert + the whole transaction
        // are protected.
        let token = unsafe { critical_section::acquire() };
        self.restore = Some(token);
        set_phase(self.second_transition);
        self.pin.set_low()
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        let r = self.pin.set_high();
        if let Some(token) = self.restore.take() {
            unsafe { critical_section::release(token) };
        }
        r
    }
}

/// Zero-sized handle to the shared bus, implementing the byte-level SPI trait
/// the drivers consume. Only valid while a [`LockCs`] holds the section, which
/// the drivers guarantee by bracketing every transfer with CS.
pub struct BusProxy;

#[inline]
fn with_master<R>(f: impl FnOnce(&mut SharedSpiMaster) -> R) -> R {
    // SAFETY: a `LockCs` holds the critical section across the transaction, so
    // interrupts are off and there is no concurrent access to `BUS`.
    let master = unsafe { (*BUS.0.get()).as_mut().expect("shared spi not installed") };
    f(master)
}

impl FullDuplex<u8> for BusProxy {
    type Error = BusError;

    fn send(&mut self, byte: u8) -> nb::Result<(), Self::Error> {
        with_master(|m| m.send(byte))
    }

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        with_master(|m| m.read())
    }
}
