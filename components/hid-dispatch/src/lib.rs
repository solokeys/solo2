#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate delog;
generate_macros!();

pub mod app;
pub mod types;
pub mod command;
pub mod dispatch;
