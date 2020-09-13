#![cfg_attr(not(feature = "std"), no_std)]

pub mod applet;
pub mod dispatch;
pub mod types;

logging::add!(logger);