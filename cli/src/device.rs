//! Solo 2 devices, which may be in regular or bootloader mode.
use core::sync::atomic::{AtomicBool, Ordering};
use std::collections::{BTreeMap, BTreeSet};

use anyhow::anyhow;
use lpc55::bootloader::{Bootloader as Lpc55, UuidSelectable};

use crate::{apps::Admin, Firmware, Result, Select as _, Uuid, Version};
use core::fmt;

pub mod ctap;
pub mod pcsc;

/// A [SoloKeys][solokeys] [Solo 2][solo2] device, in regular mode.
///
/// From an inventory perspective, the core identifier is a UUID (16 bytes / 128 bits).
///
/// From an interface perspective, either the CTAP or PCSC transport must be available.
/// Therefore, it is an invariant that at least one is interface, and the device itself
/// implements [Transport][crate::Transport].
///
/// [solokeys]: https://solokeys.com
/// [solo2]: https://solo2.dev
pub struct Solo2 {
    ctap: Option<ctap::Device>,
    pcsc: Option<pcsc::Device>,
    locked: Option<bool>,
    uuid: Uuid,
    version: Version,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TransportPreference {
    Ctap,
    Pcsc,
}

static PREFER_CTAP: AtomicBool = AtomicBool::new(false);

impl Solo2 {
    pub fn transport_preference() -> TransportPreference {
        if PREFER_CTAP.load(Ordering::SeqCst) {
            TransportPreference::Ctap
        } else {
            TransportPreference::Pcsc
        }
    }

    pub fn prefer_ctap() {
        PREFER_CTAP.store(true, Ordering::SeqCst);
    }

    pub fn prefer_pcsc() {
        PREFER_CTAP.store(false, Ordering::SeqCst);
    }

    /// NB: Requires user tap
    pub fn into_lpc55(self) -> Result<Lpc55> {
        let mut solo2 = self;
        let uuid = solo2.uuid;
        // AGAIN: This requires user tap!
        let now = std::time::Instant::now();
        Admin::select(&mut solo2)?.maintenance().ok();
        drop(solo2);

        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut lpc55 = Lpc55::having(uuid);
        while lpc55.is_err() {
            if now.elapsed().as_secs() > 15 {
                return Err(anyhow!("User prompt to confirm maintenance timed out (or udev rules for LPC 55 mode missing)!"));
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            lpc55 = Lpc55::having(uuid);
        }

        lpc55
    }
}

impl fmt::Debug for Solo2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> core::result::Result<(), fmt::Error> {
        write!(
            f,
            "Solo 2 {:X} (CTAP: {:?}, PCSC: {:?}, Version: {} aka {})",
            &self.uuid.simple(),
            &self.ctap,
            &self.pcsc,
            &self.version.to_semver(),
            &self.version.to_calver(),
        )
    }
}

impl fmt::Display for Solo2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let transports = match (self.ctap.is_some(), self.pcsc.is_some()) {
            (true, true) => "CTAP+PCSC",
            (true, false) => "CTAP only",
            (false, true) => "PCSC only",
            _ => unreachable!(),
        };
        let lock_status = match self.locked {
            Some(true) => ", locked",
            Some(false) => ", unlocked",
            None => "",
        };
        write!(
            f,
            "Solo 2 {:X} ({}, firmware {}{})",
            &self.uuid.simple(),
            transports,
            &self.version().to_calver(),
            lock_status,
        )
    }
}

impl UuidSelectable for Solo2 {
    fn try_uuid(&mut self) -> Result<Uuid> {
        Ok(self.uuid)
    }

    fn list() -> Vec<Self> {
        // iterator/lifetime woes avoiding the explicit for loop
        let mut ctaps = BTreeMap::new();
        for mut device in ctap::list().into_iter() {
            if let Ok(uuid) = device.try_uuid() {
                ctaps.insert(uuid, device);
            }
        }
        // iterator/lifetime woes avoiding the explicit for loop
        let mut pcscs = BTreeMap::new();
        for mut device in pcsc::list().into_iter() {
            if let Ok(uuid) = device.try_uuid() {
                pcscs.insert(uuid, device);
            }
        }

        let uuids = BTreeSet::from_iter(ctaps.keys().chain(pcscs.keys()).copied());
        let mut devices = Vec::new();
        for uuid in uuids.iter() {
            // a bit roundabout, but hey, "it works".
            let mut device = Self {
                ctap: ctaps.remove(uuid),
                pcsc: pcscs.remove(uuid),
                locked: None,
                uuid: *uuid,
                version: Version {
                    major: 0,
                    minor: 0,
                    patch: 0,
                },
            };
            if let Ok(mut admin) = Admin::select(&mut device) {
                if let Ok(locked) = admin.locked() {
                    device.locked = Some(locked);
                }
            }
            if let Ok(mut admin) = Admin::select(&mut device) {
                if let Ok(version) = admin.version() {
                    device.version = version;
                    devices.push(device);
                }
            }
        }
        devices
    }
}

impl Solo2 {
    /// UUID of device.
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// Firmware version on device.
    pub fn version(&self) -> Version {
        self.version
    }

    pub fn as_ctap(&self) -> Option<&ctap::Device> {
        self.ctap.as_ref()
    }

    pub fn as_ctap_mut(&mut self) -> Option<&mut ctap::Device> {
        self.ctap.as_mut()
    }

    pub fn as_pcsc(&self) -> Option<&pcsc::Device> {
        self.pcsc.as_ref()
    }

    pub fn as_pcsc_mut(&mut self) -> Option<&mut pcsc::Device> {
        self.pcsc.as_mut()
    }
}

impl TryFrom<ctap::Device> for Solo2 {
    type Error = crate::Error;
    fn try_from(device: ctap::Device) -> Result<Solo2> {
        let mut device = device;
        let locked = Admin::select(&mut device)?.locked().ok();
        let uuid = device.try_uuid()?;
        let version = Admin::select(&mut device)?.version()?;

        Ok(Solo2 {
            ctap: Some(device),
            pcsc: None,
            locked,
            uuid,
            version,
        })
    }
}

impl TryFrom<pcsc::Device> for Solo2 {
    type Error = crate::Error;
    fn try_from(device: pcsc::Device) -> Result<Solo2> {
        let mut device = device;
        let mut admin = Admin::select(&mut device)?;
        let uuid = admin.uuid()?;
        let version = admin.version()?;
        Ok(Solo2 {
            ctap: None,
            pcsc: Some(device),
            locked: None,
            uuid,
            version,
        })
    }
}

/// A SoloKeys Solo 2 device, which may be in regular ([Solo2]) or update ([Lpc55]) mode.
///
/// Not every [pcsc::Device] is a [Device]; currently if it reacts to the SoloKeys administrative
/// [App][crate::apps::admin::App] with a valid UUID, then we treat it as such.
// #[derive(Debug, Eq, PartialEq)]
pub enum Device {
    Lpc55(Lpc55),
    Solo2(Solo2),
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Device::*;
        match self {
            Lpc55(lpc55) => write!(f, "LPC 55 {:X}", Uuid::from_u128(lpc55.uuid).simple()),
            Solo2(solo2) => solo2.fmt(f),
        }
    }
}

impl UuidSelectable for Device {
    fn try_uuid(&mut self) -> Result<Uuid> {
        Ok(self.uuid())
    }

    fn list() -> Vec<Self> {
        let lpc55s = Lpc55::list().into_iter().map(Device::from);
        let solo2s = Solo2::list().into_iter().map(Device::from);
        lpc55s.chain(solo2s).collect()
    }

    /// Fails is if zero or >1 devices have the given UUID.
    fn having(uuid: Uuid) -> Result<Self> {
        let mut candidates: Vec<Device> = Self::list()
            .into_iter()
            .filter(|card| card.uuid() == uuid)
            .collect();
        match candidates.len() {
            0 => Err(anyhow!("No usable device has UUID {:X}", uuid.simple())),
            1 => Ok(candidates.remove(0)),
            n => Err(anyhow!(
                "Multiple ({}) devices have UUID {:X}",
                n,
                uuid.simple()
            )),
        }
    }
}

impl Device {
    fn uuid(&self) -> Uuid {
        match self {
            Device::Lpc55(lpc55) => Uuid::from_u128(lpc55.uuid),
            Device::Solo2(solo2) => solo2.uuid(),
        }
    }

    /// NB: will hang if in bootloader mode and Solo 2 firmware does not
    /// come up cleanly.
    pub fn into_solo2(self) -> Result<Solo2> {
        match self {
            Device::Solo2(solo2) => Ok(solo2),
            Device::Lpc55(lpc55) => {
                let uuid = Uuid::from_u128(lpc55.uuid);
                lpc55.reboot();
                drop(lpc55);

                std::thread::sleep(std::time::Duration::from_secs(1));
                let mut solo2 = Solo2::having(uuid);
                while solo2.is_err() {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    solo2 = Solo2::having(uuid);
                }

                solo2
            }
        }
    }

    /// NB: Requires user tap if device is in Solo2 mode.
    pub fn into_lpc55(self) -> Result<Lpc55> {
        match self {
            Device::Lpc55(lpc55) => Ok(lpc55),
            Device::Solo2(solo2) => solo2.into_lpc55(),
        }
    }

    pub fn program(
        self,
        firmware: Firmware,
        skip_major_prompt: bool,
        progress: Option<&dyn Fn(usize)>,
    ) -> Result<()> {
        // If device is in Solo2 mode
        // - if firmware is major version bump, confirm with dialogue
        // - prompt user tap and get into bootloader
        // let device_version: Version = admin.version()?;
        // let new_version = firmware.version();

        let lpc55 = match self {
            Device::Lpc55(lpc55) => lpc55,
            Device::Solo2(solo2) => {
                // If device is in Solo2 mode
                // - if firmware is major version bump, confirm with dialogue
                // - prompt user tap and get into Lpc55 bootloader

                info!("device fw version: {}", solo2.version.to_calver());
                info!("new fw version: {}", firmware.version().to_calver());

                if solo2.version > firmware.version() {
                    println!("Firmware version on device higher than firmware version used.");
                    println!("This would be rejected by the device.");
                    return Err(anyhow!("Firmware rollback attempt"));
                }

                // 2.964.0 is the last firmware that shipped oath-authenticator 0.1;
                // newer firmware uses secrets-app with an incompatible on-disk format.
                // Warn the user about their OATH/credential situation accordingly.
                // TODO: finalize the exact wording of these messages.
                let oath_migration_version = Version {
                    major: 2,
                    minor: 964,
                    patch: 0,
                };
                if solo2.version == oath_migration_version {
                    println!();
                    println!("WARNING: oath migration notice (placeholder message for 2.964.0).");
                    println!();
                } else if solo2.version < oath_migration_version {
                    println!();
                    println!("WARNING: updating from this version will lose ALL credentials, both FIDO and OATH.");
                    println!();
                }

                let fw_major = firmware.version().major;
                let major_version_bump = fw_major > solo2.version.major;
                if !skip_major_prompt && major_version_bump {
                    use dialoguer::{theme, Confirm};
                    println!("Warning: This is is major update and it could risk breaking any current credentials on your key.");
                    println!("Check latest release notes here to double check: https://github.com/solokeys/solo2/releases");
                    println!(
                        "If you haven't used your key for anything yet, you can ignore this.\n"
                    );

                    if Confirm::with_theme(&theme::ColorfulTheme::default())
                        .with_prompt("Continue?")
                        .wait_for_newline(true)
                        .interact()?
                    {
                        println!("Continuing");
                    } else {
                        return Err(anyhow!("User aborted."));
                    }
                }

                println!("Tap button on key to confirm, or replug to abort...");
                Self::Solo2(solo2).into_lpc55()
                    .inspect_err(|_e| {
                        if std::env::consts::OS == "linux" {
                            println!("\nIf you touched the key and the LED is off, you are likely missing udev rules for LPC 55 mode.");
                            println!("Either run `sudo solo2 update`, or install <https://github.com/solokeys/solo2-cli/blob/main/70-solo2.rules>");
                            println!("Specifically, you need this line:");
                            // SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1209", ATTRS{idProduct}=="b000", TAG+="uaccess"
                            println!(r#"SUBSYSTEM=="hidraw", ATTRS{{idVendor}}=="1209", ATTRS{{idProduct}}=="b000", TAG+="uaccess""#);
                            println!();
                        }
                    })?
            }
        };

        println!("LPC55 Bootloader detected. The LED should be off.");
        println!("Writing new firmware...");
        firmware.write_to(&lpc55, progress);

        println!("Done. Rebooting key. The LED should turn back on.");
        Self::Lpc55(lpc55).into_solo2().map(drop)
    }
}

impl From<Lpc55> for Device {
    fn from(lpc55: Lpc55) -> Device {
        Device::Lpc55(lpc55)
    }
}

impl From<Solo2> for Device {
    fn from(solo2: Solo2) -> Device {
        Device::Solo2(solo2)
    }
}
