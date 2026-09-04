//! Middleware to use the Trussed apps on a Solo 2 device.

use hex_literal::hex;

use crate::{Result, Transport};

/// Temporarily wrap an exclusive pointer to a transport, after selecting the app.
///
/// If instead apps were traits on transports - where would we store the app ID?
#[macro_export]
macro_rules! app(
    () => {

        pub struct App<'t> {
            #[allow(dead_code)]
            transport: &'t mut dyn $crate::Transport,
        }

        impl<'t> From<&'t mut dyn $crate::Transport> for App<'t> {
            fn from(transport: &'t mut dyn $crate::Transport) -> App<'t> {
                Self { transport }
            }
        }
    }
);

#[macro_export]
macro_rules! ctap_app(
    () => {

        pub struct App<'t> {
            #[allow(dead_code)]
            transport: &'t mut $crate::device::ctap::Device,
        }

        impl<'t> From<&'t mut $crate::device::ctap::Device> for App<'t> {
            fn from(transport: &'t mut $crate::device::ctap::Device) -> App<'t> {
                Self { transport }
            }
        }

        impl<'t> core::ops::Deref for App<'t> {
            type Target = $crate::device::ctap::Device;
            fn deref(&self) -> &Self::Target {
                self.transport
            }
        }

        impl<'t> core::ops::DerefMut for App<'t> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.transport
            }
        }
    }
);

#[macro_export]
macro_rules! pcsc_app(
    () => {

        pub struct App<'t> {
            #[allow(dead_code)]
            transport: &'t mut $crate::device::pcsc::Device,
        }

        impl<'t> From<&'t mut $crate::device::pcsc::Device> for App<'t> {
            fn from(transport: &'t mut $crate::device::pcsc::Device) -> App<'t> {
                Self { transport }
            }
        }

        impl<'t> core::ops::Deref for App<'t> {
            type Target = $crate::device::pcsc::Device;
            fn deref(&self) -> &Self::Target {
                self.transport
            }
        }

        impl<'t> core::ops::DerefMut for App<'t> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.transport
            }
        }
    }
);

/// App over the Ledger-style HID transport (the wallet app).
#[macro_export]
macro_rules! wallet_app(
    () => {
        pub struct App {
            device: $crate::transport::wallet::Device,
        }

        impl App {
            pub fn open(uuid: $crate::Uuid) -> $crate::Result<Self> {
                let device = $crate::transport::wallet::Device::open_wallet(uuid)?;
                Ok(Self { device })
            }
        }

        impl core::ops::Deref for App {
            type Target = $crate::transport::wallet::Device;
            fn deref(&self) -> &Self::Target {
                &self.device
            }
        }

        impl core::ops::DerefMut for App {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.device
            }
        }
    }
);

pub mod wallet;
pub use wallet::App as Wallet;
pub mod admin;
pub use admin::App as Admin;
pub mod fido;
pub use fido::App as Fido;
pub mod ndef;
pub use ndef::App as Ndef;
pub mod oath;
pub use oath::App as Oath;
pub mod oath_migrate;
pub use oath_migrate::App as OathMigrate;
pub mod piv;
pub use piv::App as Piv;
pub mod provision;
pub mod qa;

/// well-known Registered Application Provider Identifiers.
pub struct Rid;
impl Rid {
    pub const NFC_FORUM: &'static [u8] = &hex!("D276000085");
    pub const NIST: &'static [u8] = &hex!("A000000308");
    pub const SOLOKEYS: &'static [u8] = &hex!("A000000847");
    pub const YUBICO: &'static [u8] = &hex!("A000000527");
}

/// well-known Proprietary Application Identifier Extensions.
pub struct Pix;
impl Pix {
    pub const ADMIN: &'static [u8] = &hex!("00000001");
    pub const NDEF: &'static [u8] = &hex!("0101");
    pub const OATH: &'static [u8] = &hex!("2101");
    // oath-export migration app (legacy oath-authenticator 0.1 -> age export)
    pub const OATH_MIGRATE: &'static [u8] = &hex!("01000002");
    // the full PIX ends with 0100 for version 01.00,
    // truncated is enough to select
    // pub const PIV_VERSIONED: &'static [u8] = &hex!("000010000100");
    pub const PIV: &'static [u8] = &hex!("00001000");
    pub const PROVISION: &'static [u8] = &hex!("01000001");
    pub const QA: &'static [u8] = &hex!("01000000");
}

pub trait PcscSelect<'t>: From<&'t mut crate::device::pcsc::Device> {
    const RID: &'static [u8];
    const PIX: &'static [u8];

    fn application_id() -> Vec<u8> {
        let mut aid: Vec<u8> = Default::default();
        aid.extend_from_slice(Self::RID);
        aid.extend_from_slice(Self::PIX);
        aid
    }

    fn select(transport: &'t mut crate::device::pcsc::Device) -> Result<Self> {
        transport.select(Self::application_id())?;
        Ok(Self::from(transport))
    }
}

pub trait Select<'t>: From<&'t mut dyn Transport> {
    const RID: &'static [u8];
    const PIX: &'static [u8];

    fn application_id() -> Vec<u8> {
        let mut aid: Vec<u8> = Default::default();
        aid.extend_from_slice(Self::RID);
        aid.extend_from_slice(Self::PIX);
        aid
    }

    fn select(transport: &'t mut dyn Transport) -> Result<Self> {
        transport.select(Self::application_id())?;
        Ok(Self::from(transport))
    }
}
