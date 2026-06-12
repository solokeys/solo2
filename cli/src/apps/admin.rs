use crate::{Result, Uuid, Version};

crate::app!();

impl<'t> crate::Select<'t> for App<'t> {
    const RID: &'static [u8] = super::Rid::SOLOKEYS;
    const PIX: &'static [u8] = super::Pix::ADMIN;
}

impl App<'_> {
    pub const BOOT_TO_BOOTROM_COMMAND: u8 = 0x51;
    pub const REBOOT_COMMAND: u8 = 0x53;
    pub const VERSION_COMMAND: u8 = 0x61;
    pub const UUID_COMMAND: u8 = 0x62;
    pub const WINK_COMMAND: u8 = 0x08;
    pub const LOCKED_COMMAND: u8 = 0x63;

    /// Reboot the Solo 2 to maintenance mode (LPC 55 bootloader).
    ///
    /// NOTE: This command requires user confirmation (by tapping the device).
    /// Current firmware implementation has no timeout, so if the user aborts
    /// the operation host-side, the device is "stuck" until replug.
    ///
    /// Rebooting can cause the connection to return error, which should
    /// be special-cased by the caller.
    pub fn maintenance(&mut self) -> Result<()> {
        self.transport
            .instruct(Self::BOOT_TO_BOOTROM_COMMAND)
            .map(drop)
    }

    /// Reboot the Solo 2 normally.
    ///
    /// Rebooting can cause the connection to return error, which should
    /// be special-cased by the caller.
    pub fn reboot(&mut self) -> Result<()> {
        self.transport.instruct(Self::REBOOT_COMMAND).map(drop)
    }

    /// The UUID of the device.
    ///
    /// This can be fetched in multiple other ways, and is also visible in bootloader mode.
    /// Responding successfully to this command is our criterion for treating a smartcard
    /// as a Solo 2 device.
    ///
    /// NB: In early firmware, this command isn't implemented on the CTAP transport.
    pub fn uuid(&mut self) -> Result<Uuid> {
        let version_bytes = self.transport.instruct(Self::UUID_COMMAND)?;
        let bytes: &[u8] = &version_bytes;
        let _bytes_array: [u8; 16] = bytes.try_into().unwrap();
        Ok(Uuid::from_u128(
            bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("expected 16 byte UUID, got {}", &hex::encode(bytes)))
                .map(u128::from_be_bytes)?,
        ))
    }

    /// The version of the [Firmware][crate::Firmware] currently running on the Solo 2.
    pub fn version(&mut self) -> Result<Version> {
        let version_bytes = self.transport.instruct(Self::VERSION_COMMAND)?;
        let bytes: [u8; 4] = version_bytes.as_slice().try_into().map_err(|_| {
            anyhow::anyhow!(
                "expected 4 bytes version, got {}",
                &hex::encode(version_bytes)
            )
        })?;
        Ok(bytes.into())
    }

    /// Send the wink command (which fido-authenticator does not implement).
    pub fn wink(&mut self) -> Result<()> {
        self.transport.instruct(Self::WINK_COMMAND).map(drop)
    }

    pub fn locked(&mut self) -> Result<bool> {
        let locked = self.transport.instruct(Self::LOCKED_COMMAND)?;
        locked
            .first()
            .map(|&locked| locked == 1)
            .ok_or_else(|| anyhow::anyhow!("response to locked status empty"))
    }
}
