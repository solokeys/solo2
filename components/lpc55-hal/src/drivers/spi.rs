///! There are 8 "normal" SPIs and on high-speed SPI.
///! The high-speed SPI is tied to Flexcomm8, which it does not
///! share with any other peripherals.
///!
///! SPI3, SPI4, and this high-speed SPI8 have 4 possible chip selects,
///! whereas the others have two.
///
///
use core::marker::PhantomData;

pub use crate::traits::wg::spi::{
    FullDuplex,
    Mode,
    Phase,
    Polarity,
};
use crate::typestates::pin::{
    flexcomm::{
        // Trait marking I2C peripherals and pins
        Spi,
        SpiPins,
        ChipSelect,
    },
    PinId,
};
use crate::time::{
    Hertz,
};

pub mod prelude {
    pub use super::SpiMaster;
    pub use super::Error as SpiError;
    pub use super::Result as SpiResult;
}

/// SPI error
/// TODO: Use the actual ones from the chip
#[derive(Debug)]
pub enum Error {
    /// Overrun occurred
    Overrun,
    /// Mode fault occurred
    ModeFault,
    /// CRC error
    Crc,
    #[doc(hidden)]
    _Extensible,
}

pub type Result<T> = nb::Result<T, Error>;

/// SPI peripheral operating in master mode
pub struct SpiMaster<SCK, MOSI, MISO, CS, SPI, PINS>
where
    SCK: PinId,
    MOSI: PinId,
    MISO: PinId,
    CS: PinId,
    SPI: Spi,
    PINS: SpiPins<SCK, MOSI, MISO, CS, SPI>,
{
    spi: SPI,
    pins: PINS,
    _sck: PhantomData<SCK>,
    _mosi: PhantomData<MOSI>,
    _miso: PhantomData<MISO>,
    _cs: PhantomData<CS>,
    cs: ChipSelect,
}

impl<SCK, MOSI, MISO, CS, SPI, PINS> SpiMaster<SCK, MOSI, MISO, CS, SPI, PINS>
where
    SCK: PinId,
    MOSI: PinId,
    MISO: PinId,
    CS: PinId,
    SPI: Spi,
    PINS: SpiPins<SCK, MOSI, MISO, CS, SPI>,
    // CSPIN: SpiSselPin<CS, SPI>,
{
    pub fn new<Speed: Into<Hertz>>(spi: SPI, pins: PINS, speed: Speed, mode: Mode) -> Self {
        let speed: Hertz = speed.into();
        let speed: u32 = speed.0;

        while spi.stat.read().mstidle().bit_is_clear() { continue; }

        spi.fifocfg.modify(|_, w| w
            .enabletx().disabled()
            .enablerx().disabled()
        );
        spi.cfg.modify(|_, w| w
            .enable().disabled()
            .master().master_mode()
            .lsbf().standard() // MSB first
            .cpha().bit(mode.phase == Phase::CaptureOnSecondTransition)
            .cpol().bit(mode.polarity == Polarity::IdleHigh)
            .loop_().disabled()
        );

        let div: u32 = 12_000_000 / speed - 1;
        debug_assert!(div <= 0xFFFF);
        spi.div.modify(|_, w| unsafe { w.divval().bits(div as u16) } );

        // spi.raw.fifowr.write(|w| w
        //     .rxignore().ignore() // otherwise transmit halts if FIFORD buffer is full
        // );
        // spi.raw.fifotrig.modify(|_, w| w
        //     .enabletx().enabled()
        //     .enablerx().enabled()
        // );
        spi.fifocfg.modify(|_, w| w
            .enabletx().enabled()
            .enablerx().enabled()
        );
        spi.cfg.modify(|_, w| w
            .enable().enabled()
        );
        // match pins.3.CS {
        //     0...3 => {},
        //     _ => { panic!() },
        // }

        Self {
            spi,
            pins,
            _sck: PhantomData,
            _mosi: PhantomData,
            _miso: PhantomData,
            _cs: PhantomData,
            // _cs_pin: PhantomData,
            cs: PINS::CS,
        }
    }

    pub fn release(self) -> (SPI, PINS) {
        (self.spi, self.pins)
    }

    fn return_on_error(&self) -> Result<()> {
        // TODO: error readout
        Ok(())
    }

}

impl<SCK, MOSI, MISO, CS, SPI, PINS> FullDuplex<u8> for SpiMaster<SCK, MOSI, MISO, CS, SPI, PINS>
where
    SCK: PinId,
    MOSI: PinId,
    MISO: PinId,
    CS: PinId,
    SPI: Spi,
    PINS: SpiPins<SCK, MOSI, MISO, CS, SPI>,
    // CSPIN: SpiSselPin<CS, SPI>,
{
    type Error = Error;

    fn read(&mut self) -> Result<u8> {
        // self.return_on_error()?;
        if self.spi.fifostat.read().rxnotempty().bit_is_set() {
            // TODO: not sure how to turn this from u32 (or u16??) into u8
            // Or whatever...
            let byte = self.spi.fiford.read().rxdata().bits();
            Ok(byte as u8)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    fn send(&mut self, byte: u8) -> Result<()> {

        // NB: UM says "Do not read-modify-write the register."
        // - writing 0 to upper-half word means: keep previous control settings

        self.return_on_error()?;
        if self.spi.fifostat.read().txnotfull().bit_is_set() {
            // NB: we set 8 bits in constructor
            // We could probably repeat this here
            use ChipSelect::*;
            match self.cs {
                Chip0 =>  {
                    self.spi.fifowr.write(|w| unsafe { w
                        // control
                        .len().bits(7) // 8 bits
                        .txssel0_n().asserted()
                        // data
                        .txdata().bits(byte as u16)
                    });
                },
                Chip1 =>  {
                    self.spi.fifowr.write(|w| unsafe { w
                        // control
                        .len().bits(7) // 8 bits
                        .txssel1_n().asserted()
                        // data
                        .txdata().bits(byte as u16)
                    });
                },
                Chip2 =>  {
                    self.spi.fifowr.write(|w| unsafe { w
                        // control
                        .len().bits(7) // 8 bits
                        .txssel2_n().asserted()
                        // data
                        .txdata().bits(byte as u16)
                    });
                },
                Chip3 =>  {
                    self.spi.fifowr.write(|w| unsafe { w
                        // control
                        .len().bits(7) // 8 bits
                        .txssel3_n().asserted()
                        // data
                        .txdata().bits(byte as u16)
                    });
                },
                NoChips =>  {
                    self.spi.fifowr.write(|w| unsafe { w
                        // control
                        .len().bits(7) // 8 bits
                        // data
                        .txdata().bits(byte as u16)
                    });
                },
            }
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

impl<SCK, MOSI, MISO, CS, SPI, PINS> crate::traits::wg::blocking::spi::transfer::Default<u8>
for
    SpiMaster<SCK, MOSI, MISO, CS, SPI, PINS>
where
    SCK: PinId,
    MOSI: PinId,
    MISO: PinId,
    CS: PinId,
    SPI: Spi,
    PINS: SpiPins<SCK, MOSI, MISO, CS, SPI>,
{}

impl<SCK, MOSI, MISO, CS, SPI, PINS> crate::traits::wg::blocking::spi::write::Default<u8>
for
    SpiMaster<SCK, MOSI, MISO, CS, SPI, PINS>
where
    SCK: PinId,
    MOSI: PinId,
    MISO: PinId,
    CS: PinId,
    SPI: Spi,
    PINS: SpiPins<SCK, MOSI, MISO, CS, SPI>,
{}

// impl<SPI, PINS> crate::traits::wg::blocking::spi::transfer::Default<u8> for SpiMaster<SPI, PINS>
// where
//     SPI: Spi
// {}

// impl<SPI, PINS> embedded_hal::blocking::spi::write::Default<u8> for SpiMaster<SPI, PINS>
// where
//     SPI: Deref<Target = spi1::RegisterBlock>,
// {}

