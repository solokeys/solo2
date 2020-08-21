#![cfg_attr(not(feature = "std"), no_std)]

pub mod app;
pub mod types;
pub mod command;
pub mod dispatch;

logging::add!(logger);