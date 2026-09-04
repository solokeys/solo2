//! Ledger HID transport implementation for the wallet app

use crate::{Result, Uuid};
use anyhow::anyhow;
use std::cmp::min;

const APDU_TAG: u8 = 0x05;
const APDU_CLA: u8 = 0xe0;
const APDU_PAYLOAD_HEADER_LEN: usize = 7;
const LEDGER_TRANSPORT_HEADER_LEN: usize = 5;
const HID_PACKET_SIZE: usize = 64;

#[cfg(windows)]
const HID_PREFIX_ZERO: usize = 1;
#[cfg(not(windows))]
const HID_PREFIX_ZERO: usize = 0;

/// Ledger HID session (holds HidApi instance)
pub struct Session {
    api: hidapi::HidApi,
}

impl Session {
    /// Create a new session
    pub fn new() -> Result<Self> {
        Ok(Self {
            api: hidapi::HidApi::new()?,
        })
    }
}

/// Ledger HID device
pub struct Device {
    _session: Session,
    device: hidapi::HidDevice,
}

impl Device {
    /// Open a Ledger device by VID/PID
    pub fn open(vid: u16, pid: u16) -> Result<Self> {
        let session = Session::new()?;
        let device = session.api.open(vid, pid)?;
        Ok(Self {
            _session: session,
            device,
        })
    }

    /// Find and open the wallet HID interface for a Solo 2 UUID.
    pub fn open_wallet(uuid: Uuid) -> Result<Self> {
        const WALLET_USAGE_PAGE: u16 = 0xFFA0;

        let session = Session::new()?;
        let path = session
            .api
            .device_list()
            .find(|device| {
                device.usage_page() == WALLET_USAGE_PAGE
                    && device
                        .serial_number()
                        .and_then(|serial| Uuid::parse_str(serial).ok())
                        == Some(uuid)
            })
            .map(|device| device.path().to_owned())
            .ok_or_else(|| anyhow!("No wallet HID interface for UUID {:X} found", uuid.simple()))?;
        let device = session.api.open_path(&path)?;
        Ok(Self {
            _session: session,
            device,
        })
    }

    /// Write an APDU command using Ledger HID protocol
    fn write(&self, command: u8, p1: u8, p2: u8, data: &[u8]) -> Result<()> {
        let data_len = data.len();
        let mut offset = 0;
        let mut sequence_number = 0;
        let mut hid_chunk = [0_u8; HID_PACKET_SIZE + HID_PREFIX_ZERO];

        while sequence_number == 0 || offset < data_len {
            let header = if sequence_number == 0 {
                LEDGER_TRANSPORT_HEADER_LEN + APDU_PAYLOAD_HEADER_LEN
            } else {
                LEDGER_TRANSPORT_HEADER_LEN
            };
            let size = min(HID_PACKET_SIZE - header, data_len - offset);

            {
                let chunk = &mut hid_chunk[HID_PREFIX_ZERO..];
                chunk[0..5].copy_from_slice(&[
                    0x01,
                    0x01,
                    APDU_TAG,
                    (sequence_number >> 8) as u8,
                    (sequence_number & 0xff) as u8,
                ]);

                if sequence_number == 0 {
                    let data_len = data.len() + 5;
                    chunk[5..12].copy_from_slice(&[
                        (data_len >> 8) as u8,
                        (data_len & 0xff) as u8,
                        APDU_CLA,
                        command,
                        p1,
                        p2,
                        data.len() as u8,
                    ]);
                }

                chunk[header..header + size].copy_from_slice(&data[offset..offset + size]);
            }

            let n = self.device.write(&hid_chunk[..])?;
            if n < size + header + HID_PREFIX_ZERO {
                return Err(anyhow!("Write data size mismatch"));
            }
            offset += size;
            sequence_number += 1;
            if sequence_number >= 0xffff {
                return Err(anyhow!("Maximum sequence number reached"));
            }
        }
        Ok(())
    }

    /// Read an APDU response using Ledger HID protocol
    fn read(&self) -> Result<Vec<u8>> {
        let mut message_size = 0;
        let mut message = Vec::new();

        // Read timeout: 5 seconds (5000 milliseconds)
        const READ_TIMEOUT_MS: i32 = 5000;

        for chunk_index in 0..=0xffff {
            let mut chunk: [u8; HID_PACKET_SIZE + HID_PREFIX_ZERO] =
                [0; HID_PACKET_SIZE + HID_PREFIX_ZERO];
            let chunk_size = match self
                .device
                .read_timeout(&mut chunk[HID_PREFIX_ZERO..], READ_TIMEOUT_MS)
            {
                Ok(size) => size,
                Err(e) => {
                    // Handle timeout or other read errors
                    if chunk_index == 0 {
                        return Err(anyhow!(
                            "Failed to read response from device (timeout after {}ms): {}",
                            READ_TIMEOUT_MS,
                            e
                        ));
                    } else {
                        // If we've already read some data, this might be a timeout waiting for more chunks
                        // Check if we have a complete message
                        if message.len() == message_size && message_size > 0 {
                            break;
                        }
                        return Err(anyhow!(
                            "Failed to read chunk {} (timeout after {}ms): {}",
                            chunk_index,
                            READ_TIMEOUT_MS,
                            e
                        ));
                    }
                }
            };

            // Handle empty reads (shouldn't happen, but be defensive)
            if chunk_size == 0 {
                if chunk_index == 0 {
                    return Err(anyhow!("Empty response from device"));
                } else {
                    // If we've read some data, maybe the message is complete
                    if message.len() == message_size && message_size > 0 {
                        break;
                    }
                    return Err(anyhow!("Empty chunk {} received", chunk_index));
                }
            }

            if chunk_size < LEDGER_TRANSPORT_HEADER_LEN
                || chunk[HID_PREFIX_ZERO] != 0x01
                || chunk[HID_PREFIX_ZERO + 1] != 0x01
                || chunk[HID_PREFIX_ZERO + 2] != APDU_TAG
            {
                return Err(anyhow!("Unexpected chunk header"));
            }

            let seq = ((chunk[HID_PREFIX_ZERO + 3] as usize) << 8)
                | (chunk[HID_PREFIX_ZERO + 4] as usize);
            if seq != chunk_index {
                return Err(anyhow!("Unexpected sequence number"));
            }

            let mut offset = HID_PREFIX_ZERO + 5;
            if seq == 0 {
                if chunk_size < 7 {
                    return Err(anyhow!("Unexpected chunk header"));
                }
                message_size = ((chunk[HID_PREFIX_ZERO + 5] as usize) << 8)
                    | (chunk[HID_PREFIX_ZERO + 6] as usize);
                offset += 2;
            }
            message.extend_from_slice(&chunk[offset..HID_PREFIX_ZERO + chunk_size]);
            message.truncate(message_size);
            if message.len() == message_size {
                break;
            }
        }

        if message.len() < 2 {
            return Err(anyhow!("No status word"));
        }

        // Status word is the last 2 bytes
        let status =
            ((message[message.len() - 2] as u16) << 8) | (message[message.len() - 1] as u16);
        if status != 0x9000 {
            return Err(anyhow!("Command failed with status: {:04x}", status));
        }

        // Strip status word before returning
        let new_len = message.len() - 2;
        message.truncate(new_len);
        Ok(message)
    }

    /// Send an APDU command and receive response
    pub fn call_iso(
        &self,
        class: u8,
        instruction: u8,
        p1: u8,
        p2: u8,
        data: &[u8],
    ) -> Result<Vec<u8>> {
        if class != APDU_CLA {
            return Err(anyhow!("Invalid CLA, expected 0x{:02x}", APDU_CLA));
        }
        self.write(instruction, p1, p2, data)?;
        self.read()
    }
}
