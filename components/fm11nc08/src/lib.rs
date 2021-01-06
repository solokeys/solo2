#![no_std]

#[macro_use]
extern crate delog;
generate_macros!();

pub mod device;

pub use device::{
    FM11NC08,
    Configuration,
    Register,
};
