//! HAL wrappers around raw PAC peripherals.
//!
//! The APIs presented only implement basic functionality.
//! For more complex things, consult `hal::drivers`.
//!
//! In an RTIC setup, RTIC owns Peripherals and CorePeripherals,
//! so here use
//! ```
//! use lpc55_hal as hal;
//!
//! let syscon = hal::Syscon::from(device::SYSCON);
//! ```
//!
//! In a non-RTIC setup, to use a fully HAL-driven approach:
//! ```
//! use lpc55_hal as hal;
//!
//! let hal = hal::new()
//! let syscon = hal.syscon;
//! ```

pub mod adc;
pub mod anactrl;
pub mod casper;
pub mod ctimer;
pub mod dma;
pub mod flash;
pub mod flexcomm;
pub mod gint;
pub mod gpio;
pub mod hashcrypt;
pub mod inputmux;
pub mod iocon;
pub mod pfr;
pub mod pint;
pub mod pmc;
pub mod puf;
pub mod prince;
pub mod rng;
pub mod rtc;
pub mod syscon;
pub mod usbfs;
pub mod usbhs;
pub mod utick;
