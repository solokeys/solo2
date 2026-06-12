#![deny(warnings, trivial_casts, unused_qualifications)]

#[macro_use]
extern crate log;

pub use lpc55::{uuid::Uuid, UuidSelectable};

pub mod apps;
pub use apps::{admin::App as Admin, Select};
pub mod device;
pub use device::{Device, Solo2};
pub mod error;
pub use error::{Error, Result};
pub mod firmware;
pub use firmware::{Firmware, Version};
pub mod pki;
pub mod transport;
pub use transport::Transport;
