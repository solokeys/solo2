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

    // ── device config (admin Config protocol, shared with the Nitrokey admin-app) ──
    // Sent as the command byte directly (0x82/0x83), exactly like every other
    // admin command sends its code. These are > 0x7F, so they are NOT valid
    // 7-bit CTAPHID command codes — config is therefore smartcard-only (the
    // CCID/APDU transport carries them as the INS byte). The device matches them
    // directly in `Command::try_from` (admin-app GET_CONFIG/SET_CONFIG); there is
    // no vendor-command wrapper.
    pub const GET_CONFIG_COMMAND: u8 = 0x82;
    pub const SET_CONFIG_COMMAND: u8 = 0x83;

    /// Firmware default LED colors (`0x00RRGGBB`): idle green, UP blue.
    pub const DEFAULT_LED_IDLE: u32 = 0x0000_3F00;
    pub const DEFAULT_LED_UP: u32 = 0x0000_007F;
    /// Firmware default USB identity: SoloKeys `0x1209:0xbeee`, default strings.
    pub const DEFAULT_USB_VID: u16 = 0x1209;
    pub const DEFAULT_USB_PID: u16 = 0xbeee;

    /// Read a raw device-config value by key (e.g. `led.idle`, `usb.vid`).
    /// Numeric values come back as `0x`-hex, matching what `set_config` accepts.
    pub fn get_config(&mut self, key: &str) -> Result<String> {
        let response = self
            .transport
            .call(Self::GET_CONFIG_COMMAND, key.as_bytes())?;
        match response.split_first() {
            Some((0, value)) => Ok(String::from_utf8_lossy(value).into_owned()),
            Some((status, _)) => Err(anyhow::anyhow!(
                "config get failed (status 0x{:02x})",
                status
            )),
            None => Err(anyhow::anyhow!("empty response to config get")),
        }
    }

    /// Set a raw device-config value. Numeric values accept `0x`-hex or decimal;
    /// `usb.*` keys require a reboot/replug to take effect.
    pub fn set_config(&mut self, key: &str, value: &str) -> Result<()> {
        let response = self
            .transport
            .call(Self::SET_CONFIG_COMMAND, &cbor_set_request(key, value))?;
        match response.first() {
            Some(0) => Ok(()),
            Some(status) => Err(anyhow::anyhow!(
                "config set failed (status 0x{:02x})",
                status
            )),
            None => Err(anyhow::anyhow!("empty response to config set")),
        }
    }

    /// Set the idle / user-presence LED colors (`0x00RRGGBB`).
    pub fn set_led(&mut self, idle: u32, up: u32) -> Result<()> {
        self.set_config("led.idle", &format!("0x{idle:08x}"))?;
        self.set_config("led.up", &format!("0x{up:08x}"))?;
        // LED colors are read at boot — reboot so they take effect immediately
        // (same as `set_usb`). Rebooting drops the connection; ignore that.
        let _ = self.reboot();
        Ok(())
    }

    /// Set the USB identity. Empty manufacturer/product fall back to the
    /// firmware defaults.
    pub fn set_usb(&mut self, vid: u16, pid: u16, manufacturer: &str, product: &str) -> Result<()> {
        self.set_config("usb.vid", &format!("0x{vid:04x}"))?;
        self.set_config("usb.pid", &format!("0x{pid:04x}"))?;
        self.set_config("usb.manufacturer", manufacturer)?;
        self.set_config("usb.product", product)?;
        // USB identity is read at boot, so reboot to make the new VID/PID/strings
        // take effect immediately. Rebooting drops the connection — ignore that.
        let _ = self.reboot();
        Ok(())
    }
}

/// CBOR-encode the admin-app `SetConfigRequest` as the map `{"key":…,"value":…}`
/// (matching what the firmware's `cbor-smol` deserializer expects).
fn cbor_set_request(key: &str, value: &str) -> Vec<u8> {
    fn push_text(out: &mut Vec<u8>, s: &str) {
        let n = s.len();
        if n < 24 {
            out.push(0x60 | n as u8);
        } else if n < 256 {
            out.push(0x78);
            out.push(n as u8);
        } else {
            out.push(0x79);
            out.push((n >> 8) as u8);
            out.push(n as u8);
        }
        out.extend_from_slice(s.as_bytes());
    }
    let mut out = vec![0xA2]; // CBOR map of 2 pairs
    push_text(&mut out, "key");
    push_text(&mut out, key);
    push_text(&mut out, "value");
    push_text(&mut out, value);
    out
}
