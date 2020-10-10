pub enum Mode {
    Write = 0b000,
    Read = 0b001,
    WriteEeprom = 0b010,
    ReadEeprom = 0b011,
    WriteFifo = 0b100,
    ReadFifo = 0b101,
}

// bitflags::bitflags! {
//     pub struct Mode: u8 {
//         const WRITE = 0b000;
//         const READ = 0b001;
//         const WRITE_EEPROM = 0b010;
//         const READ_EEPROM = 0b011;
//         const WRITE_FIFO = 0b100;
//         const READ_FIFO = 0b101;
//     }
// }

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Register {
    #[doc(hidden)]
    FifoAccess = 0,
    /// Write this register to flush FIFO
    FifoFlush = 1,
    /// Byte count left in FIFO
    FifoCount = 2,
    /// Working status of  RF interface
    RfStatus = 3,
    /// RF response enable
    RfTxEn = 4,
    #[doc(hidden)]
    /// RF transmission baud selection
    RfBaud = 5,
    /// RATS content received by RF interface
    RfRats = 6,
    /// Main interrupt flag
    MainIrq = 7,
    /// FIFO interrupt flag
    FifoIrq = 8,
    /// Auxiliary interrupt flag
    AuxIrq = 9,
    #[doc(hidden)]
    MainIrqMask = 10,
    #[doc(hidden)]
    FifoIrqMask = 11,
    #[doc(hidden)]
    AuxIrqMask = 12,
    /// NFC configuration
    NfcConfig = 13,
    /// Regulator configuration
    RegulatorConfig = 14,
}

impl core::convert::TryFrom<u8> for Register {
    type Error = ();
    fn try_from(discriminant: u8) -> core::result::Result<Register, ()> {
        use Register::*;
        Ok(match discriminant {
            0 => FifoAccess,
            1 => FifoFlush,
            2 => FifoCount,
            3 => RfStatus,
            4 => RfTxEn,
            5 => RfBaud,
            6 => RfRats,
            7 => MainIrq,
            8 => FifoIrq,
            9 => AuxIrq,
            10 => MainIrqMask,
            11 => FifoIrqMask,
            12 => AuxIrqMask,
            13 => NfcConfig,
            14 => RegulatorConfig,
            _ => return Err(()),
        })
    }
}

// struct PixelIntoIterator {
//     pixel: Pixel,
//     index: usize,
// }

// impl IntoIterator for Register {
//     type Item = i8;
//     type IntoIter = PixelIntoIterator;

//     fn into_iter(self) -> Self::IntoIter {
//         PixelIntoIterator {
//             pixel: self,
//             index: 0,
//         }
//     }
// }

// impl core::iter::Iterator for Register {
//     type Item = Register
//     fn next(
// }

impl Register {
    pub fn spi_address(&self) -> u8 {
        *self as u8
    }
}

// bitflags::bitflags! {
//     pub struct Register: u8 {
//         const FIFO_ACCESS = 1 << 0;
//         const FIFO_FLUSH = 1 << 1;
//         const FIFO_COUNT = 1 << 2;
//         const RF_STATUS = 1 << 3;
//         const RF_TX_EN = 1 << 4;
//         const RF_BAUD = 1 << 5;
//         const RF_RATS = 1 << 6;
//         const MAIN_IRQ = 1 << 7;
//         const FIFO_IRQ = 1 << 8;
//         const AUXI_RQ = 1 << 9;
//         const MAIN_IRQ_MASK = 1 << 10;
//         const FIFO_IRQ_MASK = 1 << 11;
//         const AUX_IRQ_MASK = 1 << 12;
//         const NFC_CFG = 1 << 13;
//         const REGU_CFG = 1 << 14;
//     }
// }

bitflags::bitflags! {
    pub struct Interrupt: u8 {
        const AUX = 1 << 0;
        const FIFO = 1 << 1;
        const ARBITRATION = 1 << 2;
        const TX_DONE = 1 << 3;
        const RX_DONE = 1 << 4;
        const RX_START = 1 << 5;
        const ACTIVE = 1 << 6;
        const RF_POWER = 1 << 7;
    }
}

bitflags::bitflags! {
    pub struct FifoInterrupt: u8 {
        const EMPTY = 1 << 0;
        const FULL = 1 << 1;
        const OVERFLOW = 1 << 2;
        const WATER_LEVEL = 1 << 3;
    }
}

bitflags::bitflags! {
    pub struct AuxInterrupt: u8 {
        const FRAMING_ERROR = 1 << 3;
        const CRC_ERROR = 1 << 4;
        const PARITY_ERROR = 1 << 5;
        const FORBIDDEN_ADDRESS_ERROR = 1 << 6;
        const DONE = 1 << 7;
    }
}

bitflags::bitflags! {
    pub struct TransceptionStatus: u8 {
        const TRANSMITTING = 1 << 0;
        const RECEIVING = 1 << 1;
    }
}

pub struct Configuration {
    pub regu: u8,
    pub ataq: u16,
    pub sak1: u8,
    pub sak2: u8,
    pub tl: u8,
    pub t0: u8,
    pub ta: u8,
    pub tb: u8,
    pub tc: u8,
    pub nfc: u8,
}

/// All EEPROM configuration data.
pub struct FullConfiguration {
    pub uid: [u8; 7],
    pub regulator_cfg: u8,
    pub atqa: u16,
    pub sak1: u8,
    pub sak2: u8,
    pub tl: u8,
    pub t0: u8,
    pub ta: u8,
    pub tb: u8,
    pub tc: u8,
    pub nfc_cfg: u8,
    pub i2c_addr: u8,
    pub rblock_ack: u8,
    pub rblock_nack: u8,
}

/// State of the interrupts
pub struct InterruptState {
    pub main: u8,
    pub fifo: u8,
    pub aux: u8,
    pub count: u8,
}

/// All register data.
pub struct RegisterBlock {
    pub fifo_count: u8,
    pub rf_status: u8,
    pub rf_txen: u8,
    pub rf_baud: u8,
    pub rf_rats: u8,
    pub main_irq: u8,
    pub fifo_irq: u8,
    pub aux_irq: u8,
    pub main_irq_mask: u8,
    pub fifo_irq_mask: u8,
    pub aux_irq_mask: u8,
    pub nfc_cfg: u8,
    pub regulator_cfg: u8,
}

