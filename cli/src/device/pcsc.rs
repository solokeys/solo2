//! Device interface to Solo 2 devices.
//!
//! Might grow into an independent more idiomatic replacement for [pcsc][pcsc],
//! which is currently used internally.
//!
//! A long-shot idea is to rewrite the entire PCSC/CCID stack in pure Rust,
//! restricting to "things that are directly attached via USB", i.e. ICCD.
//!
//! Having that would allow doing the essentially same thing over HID (instead of
//! reinventing a custom HID class), or even a custom USB class (circumventing
//! the WebUSB/WebHID restrictions). Also we could easily upgrade to USB 2.0 HS,
//! instead of being stuck with full-speed USB only.
//!
//! [pcsc]: https://docs.rs/pcsc/
//!

use core::fmt;
use lpc55::bootloader::UuidSelectable;

use pcsc::{Protocols, Scope, ShareMode};

use crate::{apps::admin::App as Admin, Result, Select as _, Uuid};

/// A session with the PCSC service (running `pcscd` instance)
pub struct Session {
    session: pcsc::Context,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Info {
    /// the unique identifier for access on all platforms
    pub name: String,

    pub serial: String,
    // pub kind: String,
    pub vendor: String,
    pub version: String,
    pub atr: String,
}

/// An [ICCD][iccd] smartcard.
///
/// This object does not necessarily have a UUID, which is a Trussed/SoloKeys thing.
///
/// [iccd]: https://www.usb.org/document-library/smart-card-iccd-version-10
// #[derive(Clone)]
pub struct Device {
    pub(crate) device: pcsc::Card,
    pub name: String,
}

pub fn list() -> Vec<Device> {
    Session::new()
        .map(|session| session.devices())
        .unwrap_or_else(|_| vec![])
}

impl Session {
    /// The environment may not have an accessible PCSC service running.
    ///
    /// This performs the check.
    pub fn is_available() -> bool {
        Self::new().is_ok()
    }

    /// Establishes a user session with the PCSC service, if available.
    pub fn new() -> Result<Self> {
        Ok(Self {
            session: pcsc::Context::establish(Scope::User)?,
        })
    }

    /// Get a connection to a smartcard by name.
    ///
    /// We prefer to use normal Rust types for the smartcard names, meaning
    /// that cards with weird non-UTF8 names won't be addressable.
    pub fn connect(&self, info: &str) -> Result<Device> {
        let cstring = std::ffi::CString::new(info.as_bytes()).unwrap();
        Ok(Device {
            device: self
                .session
                .connect(&cstring, ShareMode::Shared, Protocols::ANY)?,
            name: info.to_string(),
        })
    }

    /// List all smartcards names in the system.
    ///
    /// We prefer to use normal Rust types for the smartcard names, meaning
    /// that cards with weird non-UTF8 names won't be addressable.
    pub fn infos(&self) -> Result<Vec<Info>> {
        let mut card_names_buffer = vec![0; self.session.list_readers_len()?];
        let infos = self
            .session
            .list_readers(&mut card_names_buffer)?
            .map(|name_cstr| name_cstr.to_string_lossy().to_string())
            .filter_map(|name| self.connect(&name).ok().map(|device| (name, device)))
            .map(|(name, device)| {
                Info {
                    name,
                    // kind: device.attribute(pcsc::Attribute::VendorIfdType),
                    vendor: device.attribute(pcsc::Attribute::VendorName),
                    serial: device.attribute(pcsc::Attribute::VendorIfdSerialNo),
                    version: device.attribute(pcsc::Attribute::VendorIfdVersion),
                    atr: device.attribute(pcsc::Attribute::AtrString),
                }
            })
            .collect();
        Ok(infos)
    }

    /// Get all of the usable smartcards in the system.
    pub fn devices(&self) -> Vec<Device> {
        self.infos()
            .unwrap_or_else(|_| vec![])
            .iter()
            .filter_map(|info| self.connect(&info.name).ok())
            .collect()
    }
}

impl Device {
    // serial: CStr
    // vendor: CStr
    // version: [u8] (?)
    // atr: [u8]
    fn attribute(&self, attribute: pcsc::Attribute) -> String {
        let attribute = self.device.get_attribute_owned(attribute).ok();
        attribute
            .map(|attribute| String::from_utf8_lossy(&attribute).to_string())
            .map(|mut attribute| {
                while let Some('\0') = attribute.chars().last() {
                    attribute.truncate(attribute.len() - 1);
                }
                attribute
            })
            .unwrap_or_default()
    }
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> core::result::Result<(), fmt::Error> {
        write!(f, "{}", &self.name)
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> core::result::Result<(), fmt::Error> {
        fmt::Debug::fmt(self, f)
    }
}

impl UuidSelectable for Device {
    fn try_uuid(&mut self) -> Result<Uuid> {
        let mut admin = Admin::select(self)?;
        admin.uuid()
    }

    /// Infallible method listing all usable smartcards.
    ///
    /// To find out more about issues along the way, construct the session,
    /// list the smartcards, attach them, etc.
    fn list() -> Vec<Self> {
        let session = match Session::new() {
            Ok(session) => session,
            _ => return Vec::default(),
        };
        session.devices()
    }

    // fn having(uuid: Uuid) -> Result<Self> {
    //     use super::Device as ToBeRenamed;
    //     let device = ToBeRenamed::having(uuid)?;
    //     match device {
    //         ToBeRenamed::Solo2(solo2) => Ok(solo2.into_inner()),
    //         _ => Err(anyhow!(
    //             "No smartcard found with UUID {:X}",
    //             uuid.to_simple()
    //         )),
    //     }
    // }
}
