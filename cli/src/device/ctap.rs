//! Access to CTAPHID devices
//!
//! The most convenient entry point is the `list() -> Vec<Device>` function.

use std::fmt;

use hidapi;

// use crate::{apps, Result, Uuid};
use crate::{Result, Uuid, UuidSelectable};

// This is not such a hot idea after all.
// For instance, `lpc55-host` uses `hiadpi` as well, and with
// use claiming it, will never get an instance.
//
// static SESSION: Lazy<Option<hidapi::HidApi>> = Lazy::new(|| {
//     hidapi::HidApi::new().ok()//map_err(|e| e.into())
// });

const FIDO_USAGE_PAGE: u16 = 0xF1D0;
const FIDO_USAGE: u16 = 0x1;

/// A session with the PCSC service (running `pcscd` instance)
pub struct Session {
    session: hidapi::HidApi,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Info {
    /// the unique identifier for access on all platforms
    pub path: std::ffi::CString,
    pub vid: u16,
    pub pid: u16,
    pub serial: String,
    pub manufacturer: String,
    pub product: String,
}

// #[derive(Clone)]
pub struct Device {
    pub(crate) device: hidapi::HidDevice,
    info: Info,
}

pub fn list() -> Vec<Device> {
    Session::new()
        .map(|session| session.devices())
        .unwrap_or_else(|_| vec![])
}

impl From<hidapi::DeviceInfo> for Info {
    fn from(info: hidapi::DeviceInfo) -> Self {
        Self {
            path: info.path().to_owned(),
            vid: info.vendor_id(),
            pid: info.product_id(),
            manufacturer: info.manufacturer_string().unwrap_or("").to_string(),
            product: info.product_string().unwrap_or("").to_string(),
            serial: info.serial_number().unwrap_or("").to_string(),
        }
    }
}

impl Device {
    pub fn info(&self) -> &Info {
        &self.info
    }
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.info.fmt(f)
    }
}

// // we're not trying to be thread safe here.
// // recreating the not exported lock to prevent lockup.
// // in any case, the hidapi-rs API sucks...
// static LOCK: bool = false;

// impl Drop for Session {
//     fn drop(&mut self) {
//         unsafe { LOCK = false }
//     }
// }

impl Session {
    /// TODO: Whereas in the PCSC case, the daemon may not be running
    /// e.g. in Linux, here the RW lock may already be taken.
    ///
    /// This seems dissimilar in semantics, so maybe there should not
    /// be a common name.
    pub fn is_available() -> bool {
        Self::new().is_ok()
    }

    pub fn new() -> Result<Self> {
        Ok(Self {
            session: hidapi::HidApi::new()?,
        })
    }

    pub fn infos(&self) -> Vec<Info> {
        self.session
            .device_list()
            .filter(|info| info.usage_page() == FIDO_USAGE_PAGE && info.usage() == FIDO_USAGE)
            .map(|info| info.clone().into())
            .collect()
    }

    pub fn devices(&self) -> Vec<Device> {
        self.infos()
            .into_iter()
            .filter_map(|info| {
                self.session
                    .open_path(&info.path)
                    .map(|device| Device { device, info })
                    .ok()
            })
            .collect()
    }
}

impl UuidSelectable for Device {
    /// We'd kinda like to use the `admin-app`
    fn try_uuid(&mut self) -> Result<Uuid> {
        let maybe_uuid = hex::decode(&self.info().serial)?;
        Ok(Uuid::from_slice(&maybe_uuid)?)
    }

    fn list() -> Vec<Self> {
        list()
    }

    // fn having(uuid: Uuid) -> Result<Self> {
    //     let mut candidates: Vec<Device> = Self::list()
    //         .into_iter()
    //         .filter(|card| card.uuid() == uuid)
    //         .collect();
    //     match candidates.len() {
    //         0 => Err(anyhow!("No usable device has UUID {:X}", uuid.to_simple())),
    //         1 => Ok(candidates.remove(0)),
    //         n => Err(anyhow!(
    //             "Multiple ({}) devices have UUID {:X}",
    //             n,
    //             uuid.to_simple()
    //         )),
    //     }
    // }
}
