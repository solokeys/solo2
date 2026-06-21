//! # management-app
//!
//! A simple application that implements management operations,
//! such as firmware upgrade.
//!
//! It directly implements the APDU and CTAPHID dispatch App interfaces.
#![no_std]

use trussed_core::{CryptoClient, FilesystemClient, ManagementClient, UiClient};

#[macro_use]
extern crate delog;
generate_macros!();

mod admin;
pub mod config;
pub mod migrations;

pub use admin::{App, Reboot, StatusBytes};
pub use config::{
    Config, ConfigError, ConfigField, ConfigString, ConfigValueMut, FieldType, ResetConfigResult,
    ResetSignal, ResetSignalAllocation, CONFIG_STRING_CAPACITY,
};
use trussed_manage::ManageClient;
#[cfg(feature = "se050")]
use trussed_se050_manage::Se050ManageClient;

#[cfg(not(feature = "se050"))]
pub trait Client:
    CryptoClient + FilesystemClient + ManagementClient + UiClient + ManageClient
{
}
#[cfg(not(feature = "se050"))]
impl<C: CryptoClient + FilesystemClient + ManagementClient + UiClient + ManageClient> Client for C {}

#[cfg(feature = "se050")]
pub trait Client:
    CryptoClient + FilesystemClient + ManagementClient + UiClient + Se050ManageClient + ManageClient
{
}
#[cfg(feature = "se050")]
impl<
        C: CryptoClient
            + FilesystemClient
            + ManagementClient
            + UiClient
            + Se050ManageClient
            + ManageClient,
    > Client for C
{
}
