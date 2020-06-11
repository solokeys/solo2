use fm11nc08::traits::{
    NfcDevice,
    NfcError,
};
use apdu_manager::{
    ApduSource,
    SourceError,
};

use nb::block;

use logging;
use funnel::{
    info,
};

enum Type4{
    IBlock,
    RBlock,
    SBlock,
}

/// Max pkt size is 128 for iso14443
type Packet = [u8; 128];

fn get_block_type(packet: &Packet) -> Type4 {
    let h = packet[0];

    if (h & 0xc2) == 0x02 {
        Type4::IBlock
    } else if (h & 0xe2) == 0xa2 {
        Type4::RBlock
    } else {
        Type4::SBlock
    }
}




pub struct Iso14443<DEV: NfcDevice> {
    device: DEV,
    packet: Packet,
    packet_len: u16,
    packet_payload_offset: u16,
    cid: u8,
    offset: u16,
    block_bit: u8,
    // length: u16,
}

impl<DEV> Iso14443<DEV>
where
    DEV: NfcDevice
{
    pub fn new(device: DEV) -> Self {
        Self {
            device: device,
            packet: [0u8; 128],
            packet_len: 0u16,
            packet_payload_offset: 0u16,
            offset: 0u16,
            block_bit: 0u8,
            cid: 0u8,

        }
    }

    fn load_block_settings(&mut self) {
        let header = self.packet[0];
        let mut offset = 1;


        // NAD included
        if (header & 0x4) != 0 {
            offset += 1;
        }

        // CID included
        if (header & 0x08) != 0 {
            offset += 1;
        }

        self.packet_payload_offset = offset;
        self.block_bit = header & 1;

    }

    fn ack(&mut self) {
        self.load_block_settings();

        let header = self.packet[0];
        self.packet[0] = 0xa0 | (header & 0x0f);
        self.packet[1] = self.cid;

        block!(self.device.send(
            & self.packet[0 .. self.packet_payload_offset as usize]
        )).ok();
    }

    /// Returns length of payload when finished.
    fn buffer_iblock(&mut self,buffer: &mut [u8])  -> nb::Result<u16, SourceError> {

        let h = self.packet[0];
        self.load_block_settings();

        for i in 0 .. (self.packet_len - self.packet_payload_offset) {
            buffer[self.offset as usize] = self.packet[(self.packet_payload_offset + i) as usize];
            self.offset += 1;
        }

        if (h & 0x10) != 0 {
            info!("chaining").ok();

            self.ack();

            Err(nb::Error::WouldBlock)
        } else {
            let l = self.offset;
            self.offset = 0;
            Ok(l)
        }

    }

    fn handle_blocks(&mut self, buffer: &mut [u8]) -> nb::Result<u16, SourceError> {
        match get_block_type(&self.packet) {
            Type4::IBlock => {
                return self.buffer_iblock(buffer);
            }
            Type4::RBlock => {
                self.ack();
                info!("RBlock").ok();
                Err(nb::Error::WouldBlock)
            }
            Type4::SBlock => {
                info!("SBlock??").ok();
                Err(nb::Error::Other(SourceError::NoData))
            }
        }
    }

    pub fn borrow<F: Fn(&mut DEV) -> () >(&mut self, func: F) {
        func(&mut self.device);
    }

}

impl<DEV> ApduSource for Iso14443<DEV>
where
    DEV: NfcDevice
{
    /// Read APDU into given buffer.  Return length of APDU on success.
    fn read_apdu(&mut self, buffer: &mut [u8]) -> nb::Result<u16, SourceError>{
        for _tries in 0 .. 5 {
            let res = self.device.read(&mut self.packet);
            match res {
                Ok(len) => {
                    self.packet_len = len as u16;

                    assert!(self.packet_len > 0);

                    let res = self.handle_blocks(buffer);
                    // if res.is_ok() {
                    //     info!(">> ").ok();
                    //     let l = res.ok().unwrap();
                    //     logging::dump_hex(buffer, l as usize);
                    //     return Ok(l);
                    // }

                    return res;

                }
                Err(
                    nb::Error::Other(NfcError::NoActivity)
                ) => {

                    break;
                }
                _ => {
                    // Keep going
                    return Err(nb::Error::WouldBlock);
                }
            }
        }
        return Err(nb::Error::Other(SourceError::NoData));

    }

    /// Write response code + APDU
    fn send_apdu(&mut self, code: apdu_manager::Error, buffer: &[u8]) -> nb::Result<(), SourceError>
    {
        // iblock header
        self.device.send( &[0x02 | self.block_bit] ).ok();

        if buffer.len() > 0 { self.device.send( buffer ).ok(); }
        self.device.send( &u16::to_be_bytes(code as u16) ).ok();

        // info!("<< ").ok();
        // if buffer.len() > 0 { logging::dump_hex(buffer, buffer.len()); }
        // logging::dump_hex(&u16::to_be_bytes(code as u16), 2);

        Ok(())
    }
}