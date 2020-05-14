#![allow(non_camel_case_types)]
use heapless_bytes::{
    consts,
    Unsigned as _
};

// can be 8, 16, 32, 64 or 512
#[cfg(feature = "highspeed-usb")]
pub type PACKET_SIZE_TYPE = consts::U512;
#[cfg(not(feature = "highspeed-usb"))]
pub type PACKET_SIZE_TYPE = consts::U64;

pub const PACKET_SIZE: usize = PACKET_SIZE_TYPE::USIZE;

pub const CLASS_CCID: u8 = 0x0B;
pub const SUBCLASS_NONE: u8 = 0x0;

#[repr(u8)]
pub enum TransferMode {
    // bulk transfers, optional interrupt IN
    Bulk = 0,
    // control transfers, no interrupt IN
    ControlA = 1,
    // control transfers, optional interrupt IN
    ControlB = 2,
}

pub const FUNCTIONAL_INTERFACE: u8 = 0x21;

// NB: all numbers are little-endian

// 3580 KHz (as per ICCD spec) = 3.58 MHz
// (not relevant, fixed fixed for legacy reasonse
// Yubico: 4000 KHz = 4MHz
// pub const CLOCK_FREQUENCY: [u8; 4] = 4000u32.to_le_bytes();
// instead, use Python: `import struct; struct.pack("<I", 4000)`
// pub const CLOCK_FREQUENCY_KHZ: [u8; 4] = [0xa0, 0x0f, 0x00, 0x00];
pub const CLOCK_FREQUENCY_KHZ: [u8; 4] = [0xfc, 0x0d, 0x00, 0x00];
// 9600 bps (as per ICCD spec)
// (not relevant, fixed fixed for legacy reasonse
// Yubico: 307200 bps, gnuk: 9600
// pub const DATA_RATE_BPS: [u8; 4] = [0x00, 0xb0, 0x04, 0x00];
pub const DATA_RATE_BPS: [u8; 4] = [0x80, 0x25, 0x00, 0x00];
// 254 (as per ICCD spec)
// Yubico: 2038, gnuk: 254
// pub const MAX_IFSD: [u8; 4] = [0xf6, 0x07, 0x00, 0x00];
pub const MAX_IFSD: [u8; 4] = [0xfe, 0x00, 0x00, 0x00];

// "The value shall be between 261 + 10 and 65544 + 10
// dwMaxCCIDMsgLen 3072 (gnuk: 271)
// pub const MAX_MSG_LENGTH_TYPE: consts::U3072;
pub type MAX_MSG_LENGTH_TYPE = <consts::U2048 as core::ops::Add<consts::U1024>>::Output;
pub const MAX_MSG_LENGTH: usize = MAX_MSG_LENGTH_TYPE::USIZE;
pub const MAX_MSG_LENGTH_LE: [u8; 4] = [0x00, 0x0C, 0x00, 0x00];
pub const NUM_SLOTS: u8 = 1;
pub const MAX_BUSY_SLOTS: u8 = 1;
// bPinSupport (0x0 = none, 0x01 = verification, 0x02 = modification)
pub const PIN_SUPPORT: u8 = 0;

// cf. Sec. 5.1 in: https://www.usb.org/sites/default/files/DWG_Smart-Card_CCID_Rev110.pdf
pub const FUNCTIONAL_INTERFACE_DESCRIPTOR: [u8; 52] = [
    // bcdCCID rev1.10
    0x10, 0x01,
    // bMaxSlotIndex
    NUM_SLOTS - 1,
    // bVoltageSupport (5.0V + 3.0V + 1.8V)
    0x07,
    // dwProtocols: T=1 only (0 = T=0, 3 = T0+T1)
    0x02, 0x00, 0x00, 0x00,

    // dwDefaultClock (4 MHz)
    CLOCK_FREQUENCY_KHZ[0],
    CLOCK_FREQUENCY_KHZ[1],
    CLOCK_FREQUENCY_KHZ[2],
    CLOCK_FREQUENCY_KHZ[3],
    // dwMaximumClock (same)
    CLOCK_FREQUENCY_KHZ[0],
    CLOCK_FREQUENCY_KHZ[1],
    CLOCK_FREQUENCY_KHZ[2],
    CLOCK_FREQUENCY_KHZ[3],
    // bNumClockSupported
    0x00,

    // dwDataRate (307200 bps)
    DATA_RATE_BPS[0],
    DATA_RATE_BPS[1],
    DATA_RATE_BPS[2],
    DATA_RATE_BPS[3],
    // dwMaxDataRate (same)
    DATA_RATE_BPS[0],
    DATA_RATE_BPS[1],
    DATA_RATE_BPS[2],
    DATA_RATE_BPS[3],
    // bNumDataRatesSupported
    0x00,

    // dwMaxIFSD (2038)
    MAX_IFSD[0],
    MAX_IFSD[1],
    MAX_IFSD[2],
    MAX_IFSD[3],
    // dwSyncProtocols: none
    0x00, 0x00, 0x00, 0x00,
    // dwMechanical: no special characteristics
    0x00, 0x00, 0x00, 0x00,

    // dwFeatures, see following comments
    // Auto configuration based on ATR
    // Auto activation on insert
    // Auto voltage selection
    // Auto clock change
    // Auto baud rate change
    // Auto parameter negotiation made by CCID
    // Short and extended APDU level exchange
    0xFE, 0x00, 0x04, 0x00,

    // dwMaxCCIDMsgLen (3072)
    // gnuk: 271
    MAX_MSG_LENGTH_LE[0],
    MAX_MSG_LENGTH_LE[1],
    MAX_MSG_LENGTH_LE[2],
    MAX_MSG_LENGTH_LE[3],

    // bClassGetResponse ("echo"), as per ICCD spec
    0xFF,
    // bClassEnvelope ("echo"), as per ICCD spec, gnuk: 0
    0xFF,
    // wlcdLayout (none)
    0x00, 0x00,
    // bPinSupport
    PIN_SUPPORT,
    // bMaxCCIDBusySlots
    MAX_BUSY_SLOTS,
];
