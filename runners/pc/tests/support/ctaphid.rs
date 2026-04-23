//! Minimal CTAPHID client for testing against a FIDO2 authenticator.
//!
//! Supports two transports:
//! - Unix socket: connects to the PC runner simulator
//! - USB HID: connects to a real FIDO2 device

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

const PACKET_SIZE: usize = 64;
const INIT_DATA_SIZE: usize = PACKET_SIZE - 7; // 57
const CONT_DATA_SIZE: usize = PACKET_SIZE - 5; // 59

const BROADCAST_CID: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

const CMD_INIT: u8 = 0x06 | 0x80;
const CMD_CBOR: u8 = 0x10 | 0x80;
const CMD_KEEPALIVE: u8 = 0x3B | 0x80;
const CMD_ERROR: u8 = 0x3F | 0x80;

const FIDO_USAGE_PAGE: u16 = 0xF1D0;
pub const SOCKET_PATH: &str = "/tmp/solo2-sim.sock";

enum Transport {
    Socket(UnixStream),
    Hid(hidapi::HidDevice),
}

pub struct CtapHidClient {
    transport: Transport,
    cid: [u8; 4],
}

impl CtapHidClient {
    /// Connect to the PC runner simulator over Unix socket.
    pub fn connect_socket() -> Self {
        Self::try_connect_socket()
            .unwrap_or_else(|e| panic!("Failed to connect to {}: {}", SOCKET_PATH, e))
    }

    pub fn try_connect_socket() -> Result<Self, String> {
        // Retry connection — simulator may be cycling between clients
        let stream = {
            let mut attempts = 0;
            loop {
                match UnixStream::connect(SOCKET_PATH) {
                    Ok(s) => break s,
                    Err(_e) if attempts < 20 => {
                        attempts += 1;
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => return Err(format!("connect: {e}")),
                }
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
        let mut client = CtapHidClient {
            transport: Transport::Socket(stream),
            cid: BROADCAST_CID,
        };
        client.init()?;
        Ok(client)
    }

    /// Open a real FIDO2 USB HID device.
    pub fn open_hid() -> Self {
        let api = hidapi::HidApi::new().expect("Failed to init HID API");
        let device = api
            .device_list()
            .find(|d| d.usage_page() == FIDO_USAGE_PAGE)
            .unwrap_or_else(|| panic!("No FIDO2 HID device found"))
            .open_device(&api)
            .expect("Failed to open FIDO2 HID device");
        device.set_blocking_mode(true).unwrap();
        let mut client = CtapHidClient {
            transport: Transport::Hid(device),
            cid: BROADCAST_CID,
        };
        client.init().expect("CTAPHID_INIT failed");
        client
    }

    fn init(&mut self) -> Result<(), String> {
        let nonce: [u8; 8] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let response = self
            .transact(CMD_INIT, &nonce, Duration::from_secs(5))
            .map_err(|e| format!("CTAPHID_INIT failed: {e}"))?;
        if response.len() < 17 {
            return Err(format!("INIT response too short: {}", response.len()));
        }
        self.cid.copy_from_slice(&response[8..12]);
        Ok(())
    }

    pub fn ctap2(&mut self, data: &[u8], timeout: Duration) -> Result<(u8, Vec<u8>), String> {
        let response = self.transact(CMD_CBOR, data, timeout)?;
        eprintln!(
            "[client] ctap2 response {} bytes: {:02X?}",
            response.len(),
            &response[..response.len().min(30)]
        );
        if response.is_empty() {
            return Err("Empty CBOR response".into());
        }
        Ok((response[0], response[1..].to_vec()))
    }

    fn transact(&mut self, cmd: u8, data: &[u8], timeout: Duration) -> Result<Vec<u8>, String> {
        self.send(cmd, data)?;
        self.recv(timeout)
    }

    fn send(&mut self, cmd: u8, data: &[u8]) -> Result<(), String> {
        let len = data.len();
        let mut pkt = [0u8; PACKET_SIZE];
        pkt[0..4].copy_from_slice(&self.cid);
        pkt[4] = cmd;
        pkt[5] = (len >> 8) as u8;
        pkt[6] = (len & 0xFF) as u8;
        let first_chunk = len.min(INIT_DATA_SIZE);
        pkt[7..7 + first_chunk].copy_from_slice(&data[..first_chunk]);
        self.write_packet(&pkt)?;

        let mut offset = first_chunk;
        let mut seq: u8 = 0;
        while offset < len {
            let mut cpkt = [0u8; PACKET_SIZE];
            cpkt[0..4].copy_from_slice(&self.cid);
            cpkt[4] = seq;
            let chunk = (len - offset).min(CONT_DATA_SIZE);
            cpkt[5..5 + chunk].copy_from_slice(&data[offset..offset + chunk]);
            self.write_packet(&cpkt)?;
            offset += chunk;
            seq += 1;
        }
        Ok(())
    }

    fn recv(&mut self, timeout: Duration) -> Result<Vec<u8>, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Timeout waiting for response".into());
            }

            let mut pkt = [0u8; PACKET_SIZE];
            if !self.read_packet(&mut pkt, remaining)? {
                return Err("Timeout waiting for response".into());
            }

            if pkt[0..4] != self.cid {
                continue;
            }
            if pkt[4] == CMD_KEEPALIVE {
                continue;
            }
            if pkt[4] == CMD_ERROR {
                let err = if pkt.len() > 7 { pkt[7] } else { 0xFF };
                return Err(format!("CTAPHID error: 0x{err:02X}"));
            }

            let resp_len = ((pkt[5] as usize) << 8) | (pkt[6] as usize);
            let mut data = Vec::with_capacity(resp_len);
            let first_chunk = resp_len.min(INIT_DATA_SIZE);
            data.extend_from_slice(&pkt[7..7 + first_chunk]);

            let mut seq: u8 = 0;
            while data.len() < resp_len {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err("Timeout during continuation".into());
                }
                let mut cpkt = [0u8; PACKET_SIZE];
                if !self.read_packet(&mut cpkt, remaining)? {
                    return Err("Timeout during continuation".into());
                }
                if cpkt[0..4] != self.cid {
                    continue;
                }
                if cpkt[4] == CMD_KEEPALIVE {
                    continue;
                }
                if cpkt[4] != seq {
                    return Err(format!("Bad seq: expected {seq}, got {}", cpkt[4]));
                }
                let chunk = (resp_len - data.len()).min(CONT_DATA_SIZE);
                data.extend_from_slice(&cpkt[5..5 + chunk]);
                seq += 1;
            }
            return Ok(data);
        }
    }

    fn write_packet(&mut self, pkt: &[u8; PACKET_SIZE]) -> Result<(), String> {
        match &mut self.transport {
            Transport::Socket(stream) => stream
                .write_all(pkt)
                .map_err(|e| format!("Socket write: {e}")),
            Transport::Hid(device) => {
                let mut buf = [0u8; PACKET_SIZE + 1]; // report ID prefix
                buf[1..].copy_from_slice(pkt);
                device.write(&buf).map_err(|e| format!("HID write: {e}"))?;
                Ok(())
            }
        }
    }

    fn read_packet(
        &mut self,
        pkt: &mut [u8; PACKET_SIZE],
        timeout: Duration,
    ) -> Result<bool, String> {
        match &mut self.transport {
            Transport::Socket(stream) => {
                stream.set_read_timeout(Some(timeout)).ok();
                match read_exact(stream, pkt) {
                    Ok(()) => Ok(true),
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        Ok(false)
                    }
                    Err(e) => Err(format!("Socket read: {e}")),
                }
            }
            Transport::Hid(device) => {
                let n = device
                    .read_timeout(pkt, timeout.as_millis() as i32)
                    .map_err(|e| format!("HID read: {e}"))?;
                Ok(n > 0)
            }
        }
    }
}

fn read_exact(stream: &mut impl Read, buf: &mut [u8]) -> Result<(), std::io::Error> {
    let mut pos = 0;
    while pos < buf.len() {
        match stream.read(&mut buf[pos..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "closed",
                ))
            }
            Ok(n) => pos += n,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
