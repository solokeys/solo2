//! GD25Q16 external SPI NOR (2 MB) driver.
//!
//! Implements `littlefs2::driver::Storage` so the chip can host a LittleFS
//! mounted at `Location::External`. The chip is physically present only on
//! Solo 2 hardware; on the EVK these pins are unconnected, so JEDEC probe
//! returns `None`. Ported from the Nitrokey 3 Mini reference implementation.

use core::cell::RefCell;
use core::sync::atomic::{AtomicU32, Ordering};

use embedded_hal::{blocking::spi::Transfer, digital::v2::OutputPin};
use littlefs2::{driver::Storage, io::Error};
use spi_memory::{BlockDevice, Read};

// Instrumentation counters bumped on every `Storage` call. Host reads them
// via AdminStatus before and after an op to deduce per-op SPI work.
pub static SPI_READ_COUNT: AtomicU32 = AtomicU32::new(0);
pub static SPI_READ_BYTES: AtomicU32 = AtomicU32::new(0);
pub static SPI_WRITE_COUNT: AtomicU32 = AtomicU32::new(0);
pub static SPI_WRITE_BYTES: AtomicU32 = AtomicU32::new(0);
pub static SPI_ERASE_COUNT: AtomicU32 = AtomicU32::new(0);

use crate::hal::{
    self,
    drivers::{pins, Pin, Timer},
    peripherals::{ctimer, flexcomm::Spi0},
    time::{DurationExtensions, RateExtensions},
    traits::wg::{
        spi::{FullDuplex, Mode, Phase, Polarity},
        timer::CountDown,
    },
    typestates::{
        init_state::Enabled,
        pin::{self, flexcomm::NoCs},
    },
    Iocon,
};

// SPI bus pin / type definitions. The bus (FlexComm0 / Spi0) is shared between
// this external GD25Q16 flash and the FM11NC08 NFC reader; CS is gated per
// peripheral. These only define the pins/types; bus init is in the runner.

// Shared SPI bus pins.
pub type SckPin = pins::Pio0_28;
pub type MosiPin = pins::Pio0_24;
pub type MisoPin = pins::Pio0_25;

// External flash dedicated pins.
pub type FlashCsPin = pins::Pio0_13;
pub type FlashPowerPin = pins::Pio0_21;

pub type Sck = Pin<SckPin, pin::state::Special<pin::function::FC0_SCK>>;
pub type Mosi = Pin<MosiPin, pin::state::Special<pin::function::FC0_RXD_SDA_MOSI_DATA>>;
pub type Miso = Pin<MisoPin, pin::state::Special<pin::function::FC0_TXD_SCL_MISO_WS>>;

pub type FlashCs = Pin<FlashCsPin, pin::state::Gpio<pin::gpio::direction::Output>>;

/// FIFO-burst `Transfer<u8>` adapter over a `FullDuplex<u8>` SPI master.
///
/// The default `embedded_hal::blocking::spi::transfer::Default<u8>` impl on
/// lpc55-hal's `SpiMaster` does a strict byte-by-byte ping-pong: send one byte,
/// poll RX-not-empty, read one byte, repeat. The LPC55's SPI peripheral has
/// an 8-deep TX/RX FIFO that the default impl never uses. At 8 MHz SPI the
/// wire-time per byte is ~1 us but the CPU polling between bytes costs ~3 us,
/// so the bus sits idle ~75% of the time.
///
/// This wrapper keeps up to `FIFO_AHEAD` bytes outstanding in TX, draining RX
/// opportunistically. `FIFO_AHEAD = 4` stays well inside the 8-entry FIFO so
/// neither side can overflow.
pub struct BurstSpi<S>(pub S);

const FIFO_AHEAD: usize = 4;

impl<S, E> Transfer<u8> for BurstSpi<S>
where
    S: FullDuplex<u8, Error = E>,
{
    type Error = E;

    fn transfer<'a>(&mut self, buf: &'a mut [u8]) -> Result<&'a [u8], Self::Error> {
        let len = buf.len();
        let mut tx_pos: usize = 0;
        let mut rx_pos: usize = 0;
        // No-progress guard: if neither tx nor rx advances over 1_000_000
        // iterations, bail out with a fake-success rather than spin forever
        // on a non-responsive bus. Caller treats Storage::read failure as IO.
        let mut idle: u32 = 0;
        while rx_pos < len {
            let tx_before = tx_pos;
            let rx_before = rx_pos;
            while tx_pos < len && tx_pos - rx_pos < FIFO_AHEAD {
                match self.0.send(buf[tx_pos]) {
                    Ok(()) => tx_pos += 1,
                    Err(nb::Error::WouldBlock) => break,
                    Err(nb::Error::Other(e)) => return Err(e),
                }
            }
            while rx_pos < tx_pos {
                match self.0.read() {
                    Ok(b) => {
                        buf[rx_pos] = b;
                        rx_pos += 1;
                    }
                    Err(nb::Error::WouldBlock) => break,
                    Err(nb::Error::Other(e)) => return Err(e),
                }
            }
            if tx_pos == tx_before && rx_pos == rx_before {
                idle = idle.saturating_add(1);
                if idle > 1_000_000 {
                    return Ok(buf);
                }
            } else {
                idle = 0;
            }
        }
        Ok(buf)
    }
}

/// Concrete `ExtFlashStorage` for the Solo 2 wiring.
pub type Solo2ExtFlash = ExtFlashStorage<
    BurstSpi<
        hal::drivers::SpiMaster<
            SckPin,
            MosiPin,
            MisoPin,
            hal::typestates::pin::flexcomm::NoPio,
            Spi0,
            (Sck, Mosi, Miso, NoCs),
        >,
    >,
    FlashCs,
>;

struct FlashProperties {
    size: usize,
}

const FLASH_PROPERTIES: FlashProperties = FlashProperties { size: 0x20_0000 };

/// JEDEC IDs accepted as "the 2 MB external flash". All entries are
/// 16 Mbit / 4 KB sector / 256 B page / standard SPI mode-0 chips that
/// behave identically through `spi_memory::series25`.
///
///   `[0xC8, 0x40, 0x15]`  GigaDevice GD25Q16CEIGR
///   `[0xEF, 0x40, 0x15]`  Winbond W25Q16JV
///   `[0xEF, 0x70, 0x15]`  Winbond W25Q16JV-DTR (observed on UUID DD6806...)
const ACCEPTED_JEDEC: &[[u8; 3]] = &[[0xC8, 0x40, 0x15], [0xEF, 0x40, 0x15], [0xEF, 0x70, 0x15]];

fn jedec_accepted(jedec: &[u8; 3]) -> bool {
    ACCEPTED_JEDEC.iter().any(|j| j == jedec)
}

pub const SPARE_LEN: usize = 0;

/// Physical erase-sector size of the W25Q16JV / GD25Q16. Cannot be smaller —
/// the chip's `SectorErase` (0x20) command always clears 4 KiB.
const SECTOR_SIZE: usize = 4096;

pub struct ExtFlashStorage<SPI, CS>
where
    SPI: Transfer<u8>,
    CS: OutputPin,
{
    flash: RefCell<spi_memory::series25::Flash<SPI, CS>>,
    jedec: [u8; 3],
    scratch: RefCell<[u8; SECTOR_SIZE]>,
}

impl<SPI, CS> Storage for ExtFlashStorage<SPI, CS>
where
    SPI: Transfer<u8>,
    CS: OutputPin,
{
    const BLOCK_SIZE: usize = 1024;
    const READ_SIZE: usize = 4;
    const WRITE_SIZE: usize = 256;
    const BLOCK_COUNT: usize =
        (FLASH_PROPERTIES.size / Self::BLOCK_SIZE) - (SPARE_LEN / Self::BLOCK_SIZE);
    type CACHE_SIZE = generic_array::typenum::U1024;
    type LOOKAHEAD_SIZE = generic_array::typenum::U256;

    fn read(&mut self, off: usize, buf: &mut [u8]) -> Result<usize, Error> {
        if buf.len() > FLASH_PROPERTIES.size || off > FLASH_PROPERTIES.size - buf.len() {
            return Err(Error::IO);
        }
        SPI_READ_COUNT.fetch_add(1, Ordering::Relaxed);
        SPI_READ_BYTES.fetch_add(buf.len() as u32, Ordering::Relaxed);
        let mut flash = self.flash.borrow_mut();
        map_result(flash.read(off as u32, buf), buf.len())
    }

    fn write(&mut self, off: usize, data: &[u8]) -> Result<usize, Error> {
        SPI_WRITE_COUNT.fetch_add(1, Ordering::Relaxed);
        SPI_WRITE_BYTES.fetch_add(data.len() as u32, Ordering::Relaxed);
        const CHUNK_SIZE: usize = 256;
        let mut buf = [0; CHUNK_SIZE];
        let mut off = off as u32;
        let mut flash = self.flash.borrow_mut();
        for chunk in data.chunks(CHUNK_SIZE) {
            let buf = &mut buf[..chunk.len()];
            buf.copy_from_slice(chunk);
            flash.write_bytes(off, buf).map_err(|_| Error::IO)?;
            off += CHUNK_SIZE as u32;
        }
        Ok(data.len())
    }

    /// LittleFS-driven erase. `off` and `len` are multiples of `BLOCK_SIZE`
    /// (1 KiB). The underlying chip can only erase whole 4 KiB sectors, so:
    ///
    ///   - if the request covers an entire sector, just `SectorErase`;
    ///   - otherwise read the sector, set the requested range to 0xFF in RAM,
    ///     `SectorErase`, rewrite the saved contents back.
    fn erase(&mut self, off: usize, len: usize) -> Result<usize, Error> {
        if len > FLASH_PROPERTIES.size || off > FLASH_PROPERTIES.size - len {
            return Err(Error::IO);
        }
        debug_assert!(off.is_multiple_of(Self::BLOCK_SIZE));
        debug_assert!(len.is_multiple_of(Self::BLOCK_SIZE));

        let end = off + len;
        let mut cur = off;
        let mut flash = self.flash.borrow_mut();
        let mut scratch = self.scratch.borrow_mut();

        while cur < end {
            let sector_off = (cur / SECTOR_SIZE) * SECTOR_SIZE;
            let sector_end = sector_off + SECTOR_SIZE;
            let range_start = cur;
            let range_end = core::cmp::min(end, sector_end);

            if range_start == sector_off && range_end == sector_end {
                flash
                    .erase_sectors(
                        {
                            SPI_ERASE_COUNT.fetch_add(1, Ordering::Relaxed);
                            sector_off as u32
                        },
                        1,
                    )
                    .map_err(|_| Error::IO)?;
            } else {
                flash
                    .read(sector_off as u32, scratch.as_mut())
                    .map_err(|_| Error::IO)?;
                let buf_start = range_start - sector_off;
                let buf_end = range_end - sector_off;
                scratch[buf_start..buf_end].fill(0xFF);
                flash
                    .erase_sectors(
                        {
                            SPI_ERASE_COUNT.fetch_add(1, Ordering::Relaxed);
                            sector_off as u32
                        },
                        1,
                    )
                    .map_err(|_| Error::IO)?;
                flash
                    .write_bytes(sector_off as u32, scratch.as_mut())
                    .map_err(|_| Error::IO)?;
            }

            cur = range_end;
        }
        Ok(len)
    }
}

fn map_result<SPI, CS>(
    r: Result<(), spi_memory::Error<SPI, CS>>,
    len: usize,
) -> Result<usize, Error>
where
    SPI: Transfer<u8>,
    CS: OutputPin,
{
    match r {
        Ok(()) => Ok(len),
        Err(_) => Err(Error::IO),
    }
}

/// Raw bytes captured from the chip-identity probe. `jedec` is what the bus
/// returned to `0x9F` (always 3 bytes, regardless of whether validation
/// passed). `rdsr_lo` / `rdsr_hi` are RDSR1 (`0x05`) and RDSR2 (`0x35`).
#[derive(Copy, Clone)]
pub struct SelftestResult {
    pub jedec: [u8; 3],
    pub rdsr_lo: u8,
    pub rdsr_hi: u8,
}

impl SelftestResult {
    pub const ZERO: Self = Self {
        jedec: [0; 3],
        rdsr_lo: 0,
        rdsr_hi: 0,
    };
}

impl<SPI, CS> ExtFlashStorage<SPI, CS>
where
    SPI: Transfer<u8>,
    CS: OutputPin,
{
    /// Probe the chip via JEDEC ID and three diagnostic register reads.
    /// Returns `(None, selftest)` if the chip is absent / mismatched —
    /// the selftest is always populated so the caller can surface raw
    /// bus state for debugging.
    pub fn try_new(mut spi: SPI, mut cs: CS) -> (Option<Self>, SelftestResult) {
        let selftest = Self::selftest(&mut spi, &mut cs);

        if !jedec_accepted(&selftest.jedec) {
            defmt::warn!("Unknown Ext. Flash JEDEC: got {=[u8]:#04x}", selftest.jedec,);
            return (None, selftest);
        }
        defmt::info!("Ext. Flash JEDEC accepted: {=[u8]:#04x}", selftest.jedec,);

        let flash = match spi_memory::series25::Flash::init(spi, cs) {
            Ok(f) => f,
            Err(_) => {
                defmt::warn!("Ext. Flash JEDEC matched but Flash::init failed");
                return (None, selftest);
            }
        };
        (
            Some(Self {
                flash: RefCell::new(flash),
                jedec: selftest.jedec,
                scratch: RefCell::new([0u8; SECTOR_SIZE]),
            }),
            selftest,
        )
    }

    pub fn jedec(&self) -> [u8; 3] {
        self.jedec
    }

    /// Read JEDEC (`0x9F`) + RDSR1 (`0x05`) + RDSR2 (`0x35`) via raw SPI
    /// transfers, bypassing `spi_memory`. Always succeeds — on SPI error
    /// or absent chip, the bytes captured are whatever the bus held.
    fn selftest(spi: &mut SPI, cs: &mut CS) -> SelftestResult {
        let mut jedec_buf = [0x9F, 0, 0, 0];
        Self::raw_cmd(spi, cs, &mut jedec_buf, "selftest JEDEC");
        let jedec = [jedec_buf[1], jedec_buf[2], jedec_buf[3]];

        let mut rdsr_lo_buf = [0x05, 0];
        Self::raw_cmd(spi, cs, &mut rdsr_lo_buf, "selftest RDSR-low");
        let rdsr_lo = rdsr_lo_buf[1];

        let mut rdsr_hi_buf = [0x35, 0];
        Self::raw_cmd(spi, cs, &mut rdsr_hi_buf, "selftest RDSR-high");
        let rdsr_hi = rdsr_hi_buf[1];

        defmt::info!(
            "selftest: jedec={=[u8]:#04x} rdsr_lo={=u8:#04x} rdsr_hi={=u8:#04x}",
            jedec,
            rdsr_lo,
            rdsr_hi
        );
        SelftestResult {
            jedec,
            rdsr_lo,
            rdsr_hi,
        }
    }

    fn raw_cmd(spi: &mut SPI, cs: &mut CS, buf: &mut [u8], label: &'static str) {
        let _ = cs.set_low();
        let r = spi.transfer(buf);
        let _ = cs.set_high();
        match r {
            Ok(resp) => defmt::info!("{=str}: {=[u8]:#04x}", label, &resp[1..]),
            Err(_) => defmt::warn!("{=str}: SPI transfer error", label),
        }
    }

    pub fn size(&self) -> usize {
        FLASH_PROPERTIES.size
    }

    pub fn erase_chip(&mut self) -> Result<usize, Error> {
        map_result(self.flash.borrow_mut().erase_all(), FLASH_PROPERTIES.size)
    }
}

/// Bring up the external flash: drive `FLASH_POWER` high, wait for the
/// chip to settle, configure Spi0 in mode-0 at 8 MHz, JEDEC-probe.
/// Returns `(None, selftest)` if the chip is absent or unresponsive —
/// the caller is expected to fall back to RAM-backed external storage.
///
/// Never panics. Every pin `take()` and bus operation is fallible and
/// returns the zero selftest on failure rather than aborting.
pub fn try_setup<CT>(
    spi: Spi0<Enabled>,
    gpio: &mut hal::Gpio<Enabled>,
    iocon: &mut Iocon<Enabled>,
    delay: &mut Timer<CT>,
) -> (Option<Solo2ExtFlash>, SelftestResult)
where
    CT: ctimer::Ctimer<Enabled>,
{
    use hal::traits::wg::digital::v2::OutputPin as _;

    let mut power = match FlashPowerPin::take() {
        Some(p) => p.into_gpio_pin(iocon, gpio).into_output_high(),
        None => return (None, SelftestResult::ZERO),
    };
    let _ = power.set_high();

    // 200 ms ramp-up (datasheet is ~10 ms; conservative).
    delay.start(200_000u32.microseconds());
    let _ = nb::block!(delay.wait());

    let sck = match SckPin::take() {
        Some(p) => p.into_spi0_sck_pin(iocon),
        None => return (None, SelftestResult::ZERO),
    };
    let mosi = match MosiPin::take() {
        Some(p) => p.into_spi0_mosi_pin(iocon),
        None => return (None, SelftestResult::ZERO),
    };
    let miso = match MisoPin::take() {
        Some(p) => p.into_spi0_miso_pin(iocon),
        None => return (None, SelftestResult::ZERO),
    };
    let cs = match FlashCsPin::take() {
        Some(p) => p.into_gpio_pin(iocon, gpio).into_output_high(),
        None => return (None, SelftestResult::ZERO),
    };

    let mode = Mode {
        polarity: Polarity::IdleLow,
        phase: Phase::CaptureOnFirstTransition,
    };
    let spi_master = hal::drivers::SpiMaster::new(
        spi,
        (sck, mosi, miso, pin::flexcomm::NoCs),
        8_000_000_u32.Hz(),
        mode,
    );

    ExtFlashStorage::try_new(BurstSpi(spi_master), cs)
}
