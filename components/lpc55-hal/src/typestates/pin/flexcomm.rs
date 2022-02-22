//! Various traits allowing to tie all the flexcomm-related pins
//! together and have compile-time safety.
//!
//! We use "chip select" instead of "slave select" terminology.
use core::ops::Deref;

use crate::{
    raw,
};

use super::{
    PinId,
    PinType,
};

pub trait I2c: Deref<Target = raw::i2c0::RegisterBlock> {}
pub trait I2s {}
pub trait Spi: Deref<Target = raw::spi0::RegisterBlock> {}
pub trait Usart: Deref<Target = raw::usart0::RegisterBlock> {}


/// I2C serial clock
pub trait I2cSclPin<PIO, I2C> where PIO: PinId, I2C: I2c {}
/// I2C serial data
pub trait I2cSdaPin<PIO, I2C> where PIO: PinId, I2C: I2c {}

/// I2S serial clock
pub trait I2sSckPin<PIO, I2S> where PIO: PinId, I2S: I2s {}
/// I2S word select
pub trait I2sWsPin<PIO, I2S> where PIO: PinId, I2S: I2s {}
/// I2S serial data
pub trait I2sSdaPin<PIO, I2S> where PIO: PinId, I2S: I2s {}
/// I2S master clock
pub trait I2sMclkPin<PIO, I2S> where PIO: PinId, I2S: I2s {}

pub enum ChipSelect {
    Chip0,
    Chip1,
    Chip2,
    Chip3,
    NoChips,
}

/// SPI serial clock
pub trait SpiSckPin<PIO, SPI> where PIO: PinId, SPI: Spi {}
/// SPI master-out/chip-in data
pub trait SpiMosiPin<PIO, SPI> where PIO: PinId, SPI: Spi {}
/// SPI master-in/chip-out data
pub trait SpiMisoPin<PIO, SPI> where PIO: PinId, SPI: Spi {}
/// SPI chip select
pub trait SpiCsPin<PIO, SPI> where PIO: PinId, SPI: Spi { const CS: ChipSelect; }

/// Filler type for when no Mosi is necessary
pub struct NoMosi;
impl<SPI: Spi> SpiMosiPin<NoPio, SPI> for NoMosi {}
/// Filler type for when no Miso is necessary
pub struct NoMiso;
impl<SPI: Spi> SpiMisoPin<NoPio, SPI> for NoMiso {}
/// Filler type for when no Cs is necessary
pub struct NoCs;
impl<SPI: Spi> SpiCsPin<NoPio, SPI> for NoCs { const CS: ChipSelect = ChipSelect::NoChips; }

// /// SPI chip select 0
// pub trait SpiCs0Pin<PIO, SPI> where PIO: PinId, SPI: Spi { const CS: u8 = 0; }
// /// SPI chip select 1
// pub trait SpiCs1Pin<PIO, SPI> where PIO: PinId, SPI: Spi { const CS: u8 = 1; }
// /// SPI chip select 2
// pub trait SpiCs2Pin<PIO, SPI> where PIO: PinId, SPI: Spi { const CS: u8 = 2; }
// /// SPI chip select 3
// pub trait SpiCs3Pin<PIO, SPI> where PIO: PinId, SPI: Spi { const CS: u8 = 3; }

/// USART transmitter output
pub trait UsartTxPin<PIO, USART> where PIO: PinId, USART: Usart {}
/// USART receiver input
pub trait UsartRxPin<PIO, USART> where PIO: PinId, USART: Usart {}
/// USART request-to-send output
pub trait UsartRtsPin<PIO, USART> where PIO: PinId, USART: Usart {}
/// USART clear-to-send input
pub trait UsartCtsPin<PIO, USART> where PIO: PinId, USART: Usart {}
/// USART serial clock
pub trait UsartSclkPin<PIO, USART> where PIO: PinId, USART: Usart {}

pub struct NoPio;
impl PinId for NoPio {
    const PORT: usize = !0;
    const NUMBER: u8 = !0;
    const MASK: u32 = !0;
    const OFFSET: usize = !0;

    const TYPE: PinType = PinType::D;
}

// TODO: revisit this. Instead of passing in fake pins,
// write proper drivers for the use cases.
// Think about using a generic enum {Read, Write, ReadWrite}
// parameter
/// Filler type for when no Tx is necessary
pub struct NoTx;
/// Filler type for when no Rx is necessary
pub struct NoRx;
impl<USART: Usart> UsartTxPin<NoPio, USART> for NoTx {}
impl<USART: Usart> UsartRxPin<NoPio, USART> for NoRx {}


pub trait I2cPins<PIO1: PinId, PIO2: PinId, I2C: I2c> {}

impl<PIO1, PIO2, I2C, SCL, SDA> I2cPins<PIO1, PIO2, I2C> for (SCL, SDA)
where
    PIO1: PinId,
    PIO2: PinId,
    I2C: I2c,
    SCL: I2cSclPin<PIO1, I2C>,
    SDA: I2cSdaPin<PIO2, I2C>,
{}


pub trait SpiPins<PIO1: PinId, PIO2: PinId, PIO3: PinId, PIO4: PinId, SPI: Spi> {
    const CS: ChipSelect;
}

impl<PIO1, PIO2, PIO3, PIO4, SPI, SCK, MISO, MOSI, CS>
    SpiPins<PIO1, PIO2, PIO3, PIO4, SPI>
for (SCK, MOSI, MISO, CS) where
    PIO1: PinId,
    PIO2: PinId,
    PIO3: PinId,
    PIO4: PinId,
    SPI: Spi,
    SCK: SpiSckPin<PIO1, SPI>,
    MOSI: SpiMosiPin<PIO2, SPI>,
    MISO: SpiMisoPin<PIO3, SPI>,
    CS: SpiCsPin<PIO4, SPI>,
{
    const CS: ChipSelect = CS::CS;
}

pub trait UsartPins<PIO1: PinId, PIO2: PinId, USART: Usart> {}

impl<PIO1, PIO2, USART, TX, RX> UsartPins<PIO1, PIO2, USART> for (TX, RX)
where
    PIO1: PinId,
    PIO2: PinId,
    USART: Usart,
    TX: UsartTxPin<PIO1, USART>,
    RX: UsartRxPin<PIO2, USART>,
{}

// Note: Pio0_12 can be both: into_i2c_3_scl_pin() and into_i2c_6_scl_pin() [alt1 vs alt7]
//
// pin.into_I2C3_SCL_pin()
//
// what about: let scl_pin: <_, Special<I2c4, Scl>> = pins.pio1_20.into();
// what about: let scl_pin = Pin<Pio1_20, Special<I2c4, Scl>>::from(pins.pio1_20);
//
// what about... `I2cMaster(i2c, (p0_12.into(), p1_1.into()))` <-- too much magic/work in `From`?

