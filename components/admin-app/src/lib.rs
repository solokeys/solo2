//! # management-app
//!
//! A simple application that implements management operations,
//! such as firmware upgrade.
//!
//! It directly implements the APDU and CTAPHID dispatch App interfaces.
#![no_std]

pub mod management;
pub use management::*;
