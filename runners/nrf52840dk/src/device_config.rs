//! Persisted device-identity config exposed to admin-app's config mechanism
//! (`GET_CONFIG`/`SET_CONFIG`, persisted to `config` on the internal FS).
//!
//! Read by the runner at USB enumeration (vid/pid + descriptor strings). The
//! defaults reproduce the DK's stock identity (SoloKeys `0x1209:0xbeee`); a
//! `wallet` build can `SET_CONFIG usb.vid=0x2c97 usb.pid=0x7000` to emulate a
//! Ledger Nano so Solana/Ethereum host tools recognise it. Mirrors the lpc55
//! `device_config`, with the usb.* overrides gated on `wallet` (lpc55 gates on
//! `hacker`); the nRF DK has no configurable status LED, so no led fields.

use admin_app::{Config, ConfigField, ConfigString, ConfigValueMut, FieldType};
use serde::{Deserialize, Serialize};

/// USB descriptor identity.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct UsbConfig {
    pub vid: u16,
    pub pid: u16,
    /// Empty => firmware default.
    pub manufacturer: ConfigString,
    /// Empty => firmware default.
    pub product: ConfigString,
}

impl Default for UsbConfig {
    fn default() -> Self {
        Self {
            vid: 0x1209,
            pid: 0xbeee,
            manufacturer: ConfigString::new(),
            product: ConfigString::new(),
        }
    }
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceConfig {
    #[serde(default)]
    pub usb: UsbConfig,
}

const fn field(name: &'static str, requires_reboot: bool, ty: FieldType) -> ConfigField {
    ConfigField {
        name,
        requires_touch_confirmation: false,
        requires_reboot,
        destructive: false,
        ty,
    }
}

// The usb.* identity overrides are wallet-only: without the wallet they are not
// listed and SET/GET return an error.
#[cfg(feature = "wallet")]
static FIELDS: &[ConfigField] = &[
    field("usb.vid", true, FieldType::U16),
    field("usb.pid", true, FieldType::U16),
    field("usb.manufacturer", true, FieldType::Str),
    field("usb.product", true, FieldType::Str),
];
#[cfg(not(feature = "wallet"))]
static FIELDS: &[ConfigField] = &[];

impl Config for DeviceConfig {
    fn field(&mut self, key: &str) -> Option<ConfigValueMut<'_>> {
        Some(match key {
            #[cfg(feature = "wallet")]
            "usb.vid" => ConfigValueMut::U16(&mut self.usb.vid),
            #[cfg(feature = "wallet")]
            "usb.pid" => ConfigValueMut::U16(&mut self.usb.pid),
            #[cfg(feature = "wallet")]
            "usb.manufacturer" => ConfigValueMut::Str(&mut self.usb.manufacturer),
            #[cfg(feature = "wallet")]
            "usb.product" => ConfigValueMut::Str(&mut self.usb.product),
            _ => return None,
        })
    }

    fn migration_version(&self) -> Option<u32> {
        None
    }

    fn set_migration_version(&mut self, _version: u32) -> bool {
        false
    }

    fn list_available_fields(&self) -> &'static [ConfigField] {
        FIELDS
    }
}
