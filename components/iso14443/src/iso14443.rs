use fm11nc08::traits::{
    NfcDevice,
    NfcError,
};
use iso7816::{Response, Command, command::FromSliceError, Status};
use interchange::Requester;
use nb::block;
use crate::types::ApduInterchange;
use logging;
use lpc55_hal as hal;
use funnel::{
    info,
};

pub enum SourceError {
    NoData,
}

enum Type4{
    IBlock,
    RBlock,
    SBlock,
}

/// Max pkt size is 256 for iso14443
type Packet = [u8; 256];

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

    buffer: [u8; 1024],

    interchange: Requester<ApduInterchange>,
}

impl<DEV> Iso14443<DEV>
where
    DEV: NfcDevice
{
    pub fn new(device: DEV, interchange: Requester<ApduInterchange>) -> Self {
        Self {
            device: device,
            packet: [0u8; 256],
            packet_len: 0u16,
            packet_payload_offset: 0u16,
            offset: 0u16,
            block_bit: 0u8,
            cid: 0u8,

            // TODO use memory from elsewhere?
            buffer: [0u8; 1024],

            interchange: interchange,
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

        self.device.send(&[]).ok();
        self.device.send(
            & self.packet[0 .. self.packet_payload_offset as usize]
        ).ok();
        // block!(self.device.wait()).ok();
    }

    /// Returns length of payload when finished.
    fn buffer_iblock(&mut self,)  -> nb::Result<u16, SourceError> {
        let h = self.packet[0];
        self.load_block_settings();

        for i in 0 .. (self.packet_len - self.packet_payload_offset) {
            self.buffer[self.offset as usize] = self.packet[(self.packet_payload_offset + i) as usize];
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

    fn handle_blocks(&mut self) -> nb::Result<u16, SourceError> {
        match get_block_type(&self.packet) {
            Type4::IBlock => {
                return self.buffer_iblock();
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

    /// Read APDU into given buffer.  Return length of APDU on success.
    fn check_for_apdu(&mut self) -> nb::Result<(), SourceError> {
        let res = (self.device.read(&mut self.packet));
        match res {
            Ok(len) => {
                self.packet_len = len as u16;

                assert!(self.packet_len > 0);

                let l = self.handle_blocks()?;

                info!(">> {}", hal::get_cycle_count() / 96_000).ok();
                logging::dump_hex(&self.buffer, l as usize);

                match Command::try_from(&self.buffer[0 .. l as usize]) {
                    Ok(command) => {
                        self.interchange.request(command).expect("could not deposit command");
                    },
                    Err(_error) => {
                        logging::info!("apdu bad").ok();
                        match _error {
                            FromSliceError::TooShort => { info!("TooShort").ok(); },
                            FromSliceError::InvalidClass => { info!("InvalidClass").ok(); },
                            FromSliceError::InvalidFirstBodyByteForExtended => { info!("InvalidFirstBodyByteForExtended").ok(); },
                            FromSliceError::CanThisReallyOccur => { info!("CanThisReallyOccur").ok(); },
                        }

                        self.send_apdu(
                            &Response::Status(Status::UnspecifiedCheckingError).into_message()
                        )?;
                    }
                };

            }
            _ => {
            }
        }
        Ok(())
    }

    pub fn is_ready_to_transmit(&self) -> bool {
        self.interchange.state() == interchange::State::Responded
    }


    pub fn poll(&mut self){

        if let Some(response) = self.interchange.take_response() {
            self.send_apdu(
                &response.into_message()
            ).ok();
        } else {
            self.check_for_apdu().ok();
        }
    }


    /// Write response code + APDU
    fn send_apdu(&mut self, buffer: &[u8]) -> nb::Result<(), SourceError>
    {
        // iblock header
        self.device.send(&[]).ok();
        self.device.write_but_dont_send( &[0x02 | self.block_bit] );
        let r = self.device.send( buffer );
        if !r.is_ok() {
            return Err(nb::Error::Other(SourceError::NoData));
        }

        // if buffer.len() > 0 {
            // if !r.is_ok() {
                // return Err(nb::Error::Other(SourceError::NoData));
            // }
        // }

        // block!(self.device.wait()).ok();

        info!("<< {}", hal::get_cycle_count() / 96_000).ok();
        if buffer.len() > 0 { logging::dump_hex(buffer, buffer.len()); }

        Ok(())
    }

}
