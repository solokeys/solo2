//! Persisted device-identity config exposed to admin-app's config mechanism
//! (`GET_CONFIG`/`SET_CONFIG`, persisted to `config` on the internal FS).
//!
//! Read by the runner at USB enumeration (vid/pid + descriptor strings) and by
//! the status-LED UI (idle/UP colors). Defaults reproduce the stock behavior:
//! SoloKeys `0x1209:0xbeee`, product string from PFR, idle green / UP blue.

use admin_app::{Config, ConfigField, ConfigString, ConfigValueMut, FieldType};
use serde::{Deserialize, Serialize};

/// USB descriptor identity.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct UsbConfig {
    pub vid: u16,
    pub pid: u16,
    /// Empty => firmware default (`"SoloKeys"`).
    pub manufacturer: ConfigString,
    /// Empty => firmware default (product string from PFR).
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

/// Status-LED colors as `0x00RRGGBB`. `idle` is shared with Processing, `up`
/// with wink. Error is always red and not configurable.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct LedConfig {
    pub idle: u32,
    pub up: u32,
}

impl Default for LedConfig {
    fn default() -> Self {
        // Stock colors: idle green, UP blue (the breathe scales each channel).
        Self {
            idle: 0x0000_3F00,
            up: 0x0000_007F,
        }
    }
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceConfig {
    #[serde(default)]
    pub usb: UsbConfig,
    #[serde(default)]
    pub led: LedConfig,
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

// The usb.* identity overrides are hacker-only: on a secure build they are not
// listed and SET/GET return an error. The LED colors are always available.
#[cfg(feature = "hacker")]
static FIELDS: &[ConfigField] = &[
    field("usb.vid", true, FieldType::U16),
    field("usb.pid", true, FieldType::U16),
    field("usb.manufacturer", true, FieldType::Str),
    field("usb.product", true, FieldType::Str),
    field("led.idle", false, FieldType::U32),
    field("led.up", false, FieldType::U32),
];
#[cfg(not(feature = "hacker"))]
static FIELDS: &[ConfigField] = &[
    field("led.idle", false, FieldType::U32),
    field("led.up", false, FieldType::U32),
];

impl Config for DeviceConfig {
    fn field(&mut self, key: &str) -> Option<ConfigValueMut<'_>> {
        Some(match key {
            #[cfg(feature = "hacker")]
            "usb.vid" => ConfigValueMut::U16(&mut self.usb.vid),
            #[cfg(feature = "hacker")]
            "usb.pid" => ConfigValueMut::U16(&mut self.usb.pid),
            #[cfg(feature = "hacker")]
            "usb.manufacturer" => ConfigValueMut::Str(&mut self.usb.manufacturer),
            #[cfg(feature = "hacker")]
            "usb.product" => ConfigValueMut::Str(&mut self.usb.product),
            "led.idle" => ConfigValueMut::U32(&mut self.led.idle),
            "led.up" => ConfigValueMut::U32(&mut self.led.up),
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
