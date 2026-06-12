//! Simplistic CTAPHID transport protocol implementation.
//!
//! Can switch to `ctaphid` once it stabilizes.

pub use crate::{device::ctap::Device, Result};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// CTAPHID commands
pub enum Code {
    Ping,
    Init,
    Wink,
    Error,
    Keepalive,
    Vendor(VendorCode),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
/// Status conferred by a Keepalive response
pub enum Status {
    Processing,
    UserPresenceNeeded,
    Other(u8),
}

impl From<Status> for u8 {
    fn from(status: Status) -> u8 {
        use Status::*;
        match status {
            Processing => 1,
            UserPresenceNeeded => 2,
            Other(status) => status,
        }
    }
}

impl From<u8> for Status {
    fn from(status: u8) -> Self {
        use Status::*;
        match status {
            1 => Processing,
            2 => UserPresenceNeeded,
            _ => Other(status),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidCommand = 1,
    InvalidParameter = 2,
    InvalidLength = 3,
    InvalidSequenceNumber = 4,
    MessageTimeout = 5,
    ChannelBusy = 6,
    ChannelLockRequired = 0xA,
    ChannelIdInvalid = 0xB,
    UnspecifiedError = 0x7F,
}

impl From<u8> for Error {
    fn from(error: u8) -> Self {
        use Error::*;
        match error {
            1 => InvalidCommand,
            2 => InvalidParameter,
            3 => InvalidLength,
            4 => InvalidSequenceNumber,
            5 => MessageTimeout,
            6 => ChannelBusy,
            0xA => ChannelLockRequired,
            0xB => ChannelIdInvalid,
            // yes, yes, but come on ;)
            _ => UnspecifiedError,
        }
    }
}

impl From<u8> for Code {
    fn from(code: u8) -> Self {
        use Code::*;
        match code {
            0x1 => Ping,
            0x6 => Init,
            0x8 => Wink,
            0x3F => Error,
            0x3B => Keepalive,
            vendor_code @ 0x40..=0x7F => Vendor(VendorCode::new(vendor_code)),
            _ => panic!(),
        }
    }
}

impl From<Code> for u8 {
    fn from(code: Code) -> u8 {
        use Code::*;
        match code {
            Ping => 0x1,
            Init => 0x6,
            Wink => 0x8,
            Error => 0x3F,
            Keepalive => 0x3B,
            Vendor(code) => code.0,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct VendorCode(u8);
impl VendorCode {
    /// Must be at least 0x40 and at most 0x7F, else panic.
    pub const fn new(vendor_code: u8) -> Self {
        assert!(vendor_code >= 0x40);
        assert!(vendor_code <= 0x7F);
        Self(vendor_code)
    }
}

pub struct Command {
    code: Code,
    data: Vec<u8>,
}

impl Command {
    pub fn new(code: Code) -> Self {
        Self { code, data: vec![] }
    }

    pub fn with_data(self, data: &[u8]) -> Self {
        assert!(data.len() <= 7609);
        Self {
            code: self.code,
            data: data.to_vec(),
        }
    }

    pub fn packets(&self, channel: Channel) -> impl Iterator<Item = [u8; 64]> + '_ {
        use std::iter;

        let l = self.data.len();
        assert!(l <= 7609);
        let data = &self.data;
        let init_l = core::cmp::min(l, 64 - 7);
        // dbg!("init_l", init_l);

        let mut init_packet = [0u8; 64];
        init_packet[..4].copy_from_slice(&channel.0.to_be_bytes());
        init_packet[4] = u8::from(self.code) | (1 << 7);
        init_packet[5..][..2].copy_from_slice(&(l as u16).to_be_bytes());
        init_packet[7..][..init_l].copy_from_slice(&data[..init_l]);

        let init_iter = iter::once(init_packet);

        let cont_iter = data[init_l..]
            .chunks(64 - 5)
            .enumerate()
            .map(move |(i, chunk)| {
                // dbg!("cont", i, chunk.len());
                let mut cont_packet = [0u8; 64];
                cont_packet[..4].copy_from_slice(&channel.0.to_be_bytes());
                cont_packet[4] = i as u8;
                cont_packet[5..][..chunk.len()].copy_from_slice(chunk);
                cont_packet
            });

        init_iter.chain(cont_iter)
    }
}

// pub struct Packets {
//     command: Command,
// }

// impl Iterator for Packets {
//     type Item = [u8; 64];

//     fn next(
// }

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Channel(u32);

impl Channel {
    pub const BROADCAST: Self = Self(0xffff_ffff);
}

impl Device {
    pub fn call(&self, channel: Channel, request: Command) -> Result<Vec<u8>> {
        let result: Result<Vec<()>> = request
            .packets(channel)
            .map(|packet| {
                // need to prefix report ID
                let mut prefixed = vec![0];
                prefixed.extend_from_slice(&packet);
                self.device.write(&prefixed).map_err(|e| e.into()).map(drop) //|size| println!("sent {}", size))
            })
            .collect();
        result?;

        let mut packet = [0u8; 64];
        // trace!("packet: {}", hex::encode(packet));
        loop {
            let read = self.device.read(&mut packet)?;
            assert!(read >= 7);
            if packet[..4] != channel.0.to_be_bytes() {
                // got response for other channel
                continue;
            }

            if packet[4] == u8::from(Code::Keepalive) | (1 << 7) {
                let status = Status::from(packet[7]);
                info!("received keepalive, status {:?}", status);
                continue;
            }

            // the assertion on packet[4] below is is not the case for failed commands (!)
            if packet[4] == u8::from(Code::Error) | (1 << 7) {
                return Err(anyhow::anyhow!("error: {:?}", Error::from(packet[7])));
            }

            assert_eq!(packet[4], u8::from(request.code) | (1 << 7));
            break;
        }

        let l = u16::from_be_bytes(packet[5..][..2].try_into().unwrap());
        let mut data = vec![0u8; l as _];
        let init_l = core::cmp::min(l, 64 - 7) as usize;
        data[..init_l].copy_from_slice(&packet[7..][..init_l]);

        let result: Result<Vec<()>> = data[init_l..]
            .chunks_mut(64 - 5)
            .enumerate()
            .map(|(i, chunk)| {
                let read = self.device.read(&mut packet).unwrap();
                assert!(read >= 5);
                // dbg!(hex::encode(&packet[..read]));
                assert_eq!(packet[..4], channel.0.to_be_bytes());
                assert_eq!(packet[4], i as u8);
                chunk.copy_from_slice(&packet[5..][..chunk.len()]); //64- 5]);
                Ok(())
            })
            .collect();
        result?;

        // let cont_iter = data[init_l..].chunks(64 - 5).enumerate()
        //     .map(move |(i, chunk)| {
        //         let mut cont_packet = [0u8; 64];
        //         cont_packet[..4].copy_from_slice(&channel.0.to_be_bytes());
        //         cont_packet[4] = i as u8;
        //         cont_packet[5..][..chunk.len()].copy_from_slice(chunk);
        //         cont_packet
        //     });
        Ok(data)
    }

    pub fn init(&self) -> Result<Init> {
        let mut nonce = [0u8; 8];
        getrandom::getrandom(&mut nonce).unwrap();
        // dbg!(hex::encode(&nonce));
        let command = Command::new(Code::Init).with_data(&nonce);
        let response = self.call(Channel::BROADCAST, command)?;
        // let mut packet = [0u8; 64];
        // let read = self.device.read(&mut packet)?;
        assert_eq!(response.len(), 17);
        assert_eq!(response[..8], nonce);
        let version = response[12];
        assert_eq!(version, 2);
        let capabilities = response[16];

        Ok(Init {
            channel: Channel(u32::from_be_bytes(response[8..][..4].try_into().unwrap())),
            // version: response[12],
            major: response[13],
            minor: response[14],
            build: response[15],
            can_wink: (capabilities & 1) != 0,
            can_cbor: (capabilities & 4) != 0,
            can_msg: (capabilities & 8) == 0,
        })
        // // assert_eq!(nonce, response[..8])
        // println!("response: {}", hex::encode(&response));//&packet[..read]));
        // todo!();
    }

    pub fn ping(&self, channel: Channel, data: &[u8]) -> Result<Vec<u8>> {
        let command = Command::new(Code::Ping).with_data(data);
        let response = self.call(channel, command)?;

        assert_eq!(data, response);
        Ok(response)
    }

    pub fn wink(&self, channel: Channel) -> Result<Vec<u8>> {
        let command = Command::new(Code::Wink);
        let response = self.call(channel, command)?;
        Ok(response)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Init {
    pub channel: Channel,
    // pub version: u8,
    pub major: u8,
    pub minor: u8,
    pub build: u8,
    pub can_wink: bool,
    pub can_cbor: bool,
    pub can_msg: bool,
}
