use core::mem::MaybeUninit;
use core::time::Duration;
use heapless_bytes::Bytes;
use crate::traits::{
    nfc,
};
use iso7816::command;
use interchange::Requester;
use apdu_dispatch::types::ContactlessInterchange;

pub enum SourceError {
    NoActivity,
}

/// Returned by `.poll()`.  This returns a potential duration that
/// should be used to call `.poll_wait_extensions()` once elapsed.
/// E.g. if Duration == 40ms, then poll_wait_extensions should be called approximately 40 ms later.
/// It is up to the application how this is scheduled.
pub enum Iso14443Status {
    Idle,
    ReceivedData(Duration),
}

// Max iso14443 frame is 256 bytes
type Iso14443Frame = heapless_bytes::Bytes<heapless::consts::U256>;
type Iso7816Data = iso7816::response::Data;

#[derive(Clone, PartialEq)]
enum Iso14443State {
    Receiving,
    /// last_frame_transmitted, remaining_bytes_to_transmit.
    Transmitting(core::ops::Range<usize>, core::ops::Range<usize>),
}

type Ack = bool;
type Chaining = bool;
type BlockNum = bool;
type Offset = usize;
type WtxGranted = bool;
type Nad = Option<u8>;
type Cid = Option<u8>;

#[derive(Copy,Clone)]
enum Block {
    IBlock(BlockNum, Nad, Cid, Chaining, Offset),
    RBlock(BlockNum, Cid, Ack, Offset),
    SBlock(Cid, WtxGranted, ),
}

impl Block {
    fn new(frame: &[u8]) -> Block {
        let header = frame[0];

        let block_num = (header & 1) != 0;
        let flag = (header & 0x10) != 0;
        let mut offset = 1;

        // CID included
        let cid = if (header & 0x08) != 0 {
            offset += 1;
            Some(frame[1])
        } else {
            None
        };

        if (header & 0xc2) == 0x02 {

            // NAD included
            let nad = if (header & 0x4) != 0 {
                offset += 1;
                if cid.is_some() {
                    Some(frame[2])
                } else {
                    Some(frame[1])
                }
            } else {
                None
            };
            Block::IBlock(block_num, nad, cid, flag, offset)
        } else if (header & 0xe2) == 0xa2 {
                                    // Ack or Nack
            Block::RBlock(block_num, cid, !flag, offset)
        } else {
            Block::SBlock(cid, (0x30 & header) == 0x30)
        }
    }
}

pub struct Iso14443<DEV: nfc::Device> {
    device: DEV,

    state: Iso14443State,

    // May need to retransmit ack/Rblock to PCD
    last_iblock_recv: Option<Block>,
    // Used to see if PCD needs to have a iblock retransmitted
    last_rblock_recv: Option<Block>,
    // Used to set the new block_num on transmitted blocks
    last_block_num_recv: Option<bool>,
    // Used to see if wtx was accepted or not
    wtx_requested: bool,

    buffer: Iso7816Data,

    interchange: Requester<ContactlessInterchange>,
}

impl<DEV> Iso14443<DEV>
where
    DEV: nfc::Device
{
    pub fn new(device: DEV, interchange: Requester<ContactlessInterchange>) -> Self {
        Self {
            device: device,
            state: Iso14443State::Receiving,

            wtx_requested: false,
            last_iblock_recv: None,
            last_rblock_recv: None,
            last_block_num_recv: None,

            buffer: Bytes::new(),

            interchange: interchange,
        }
    }

    fn ack(&mut self, block: Block) {
        let mut packet = [0u8; 3];
        let length = match block {
            Block::IBlock(block_num, _nad, cid, _chaining, _offset) => {
                let header = 0xA0u8 | (block_num as u8);
                packet[0] = header;
                if let Some(cid) = cid {
                    packet[0] |= 0x08;
                    packet[1] = cid;
                    2
                } else {
                    1
                }
            }
            Block::RBlock(block_num, cid, _ack, offset) => {
                let header = 0xA0u8 | (block_num as u8);
                packet[0] = header;
                if let Some(cid) = cid {
                    packet[0] |= 0x08;
                    packet[1] = cid;
                }
                offset
            }
            _ => {
                panic!("Can only ack I or R blocks.");
            }
        };

        self.device.send(
            & packet[0 .. length]
        ).ok();
    }

    fn send_wtx(&mut self) {
        match self.last_iblock_recv.as_ref() {
            Some(Block::IBlock(_block_num, _nad, Some(cid), _chaining, _offset)) => {
                self.device.send(
                    &[0xfa, *cid, 0x01]
                ).ok();
            }
            _ => {
                self.device.send(
                    &[0xf2, 0x01]
                ).ok();
            }
        }
    }
    // IBlock(BlockNum, Nad, Cid, Chaining, ),
    // RBlock(BlockNum, Cid, Ack, ),
    // SBlock(Cid, WtxGranted, ),
    fn handle_block(&mut self, packet: &[u8]) -> Result<(), SourceError> {
        let block_header = Block::new(packet);
        match block_header {
            Block::IBlock(block_num, _nad, _cid, chaining, offset) => {

                if self.state != Iso14443State::Receiving {
                    self.buffer.clear();
                }
                self.state = Iso14443State::Receiving;

                self.buffer.extend_from_slice(& packet[offset .. ]).ok();

                self.last_iblock_recv = Some(block_header);
                self.last_block_num_recv = Some(block_num);

                if chaining {
                    self.ack(block_header);
                    Err(SourceError::NoActivity)
                } else {
                    self.wtx_requested = false;
                    Ok(())
                }

            }
            Block::RBlock(block_num, _cid, ack, _offset) => {
                if ack {

                    let duplicate_rblock = Some(block_num) == self.last_block_num_recv;
                    self.last_rblock_recv = Some(block_header);
                    self.last_block_num_recv = Some(block_num);

                    match self.state.clone() {
                        Iso14443State::Transmitting(last_frame_range, remaining_data_range) => {
                            if duplicate_rblock {
                                info!("Duplicate rblock, retransmitting");
                                self.send_frame(
                                    &Bytes::try_from_slice(
                                        &self.buffer[last_frame_range]
                                    ).unwrap()
                                ).ok();
                            } else {
                                if remaining_data_range.len() == 0 {
                                    info!("Error, recieved ack when this is no more data.");
                                    self.ack(block_header);
                                    self.reset_state();
                                    return Err(SourceError::NoActivity);
                                }
                                if let Some(last_rblock_recv) = self.last_rblock_recv {
                                    let msg = &self.buffer[remaining_data_range.clone()];
                                    let (next_frame, data_used) = self.construct_iblock(
                                        last_rblock_recv, msg
                                    );
                                    self.send_frame(&next_frame).ok();
                                    if data_used != remaining_data_range.len() {
                                        info!("Next frame");
                                        self.state = Iso14443State::Transmitting(
                                            remaining_data_range.start .. remaining_data_range.start + data_used,
                                            remaining_data_range.start + data_used .. self.buffer.len(),
                                        )
                                    } else {
                                        info!("Last frame sent!");
                                        self.state = Iso14443State::Transmitting(
                                            remaining_data_range.start .. remaining_data_range.start + data_used,
                                            self.buffer.len() .. self.buffer.len()
                                        )
                                    }

                                } else {
                                    info!("Session has been reset.");
                                    self.state = Iso14443State::Receiving;
                                }
                            }
                        }
                        _ => {
                            // (None, Iso14443State::Idle)
                            info!("Unexpected Rblock ack");
                            self.ack(block_header);
                        }
                    };

                } else {
                    if let Some(last_iblock_recv) = self.last_iblock_recv {
                        self.ack(last_iblock_recv);
                        info!("Ack last iblock");
                    } else {
                        self.ack(block_header);
                        info!("Ack ping");
                    }
                }
                Err(SourceError::NoActivity)
            }
            Block::SBlock(_cid, wtxgranted) => {
                if wtxgranted {
                    if self.wtx_requested {
                        info!("wtx accepted");
                    } else {
                        info!("unsolicited wtx");
                    }
                    self.wtx_requested = false;
                } else {
                    info!("Deselected.");
                    self.device.send(
                        &[0xc2]
                    ).ok();
                    self.reset_state();
                }
                Err(SourceError::NoActivity)
            }
        }
    }

    pub fn borrow<F: Fn(&mut DEV) -> () >(&mut self, func: F) {
        func(&mut self.device);
    }

    fn construct_iblock(&self, last_recv_block: Block, data: &[u8]) -> (Iso14443Frame, usize) {
        // iblock header
        let mut frame = Iso14443Frame::new();
        frame.push(0).ok();

        let header_length = match last_recv_block {
            Block::IBlock(block_num, nad, cid, _chaining, offset) => {
                frame[0] = 0x02u8 | (block_num as u8);
                if let Some(cid) = cid {
                    frame[0]|= 0x08;
                    frame.push(cid).ok();
                }
                if let Some(nad) = nad {
                    frame[0]|= 0x04;
                    frame.push(nad).ok();
                }
                offset
            }
            Block::RBlock(block_num, cid, _ack, offset) => {
                frame[0] = 0x02u8 | (block_num as u8);
                if let Some(cid) = cid {
                    frame[0]|= 0x08;
                    frame.push(cid).ok();
                }
                offset
            }
            _ => {
                panic!("Can only send iblock in reply to I or R blocks.");
            }
        };
        // minus 2 to leave room for crc
        let frame_size: usize = self.device.frame_size() - 2;
        let payload_len = core::cmp::min(frame_size - header_length, data.len());

        frame.extend_from_slice(&data[0 .. payload_len]).ok();

        if payload_len != data.len() {
            // set chaining bit.
            frame[0] |= 0x10;
        }

        (frame, payload_len)
    }

    fn reset_state(&mut self) {
        self.buffer.clear();
        self.state = Iso14443State::Receiving;
        self.last_iblock_recv = None;
        self.last_rblock_recv = None;
        self.last_block_num_recv = None;
        info!("state reset.");
    }

    /// Read APDU into given buffer.  Return length of APDU on success.
    fn check_for_apdu(&mut self) -> Result<(), SourceError> {
        let mut packet = MaybeUninit::<[u8; 256]>::uninit();
        let packet = unsafe { &mut *packet.as_mut_ptr() };
        // let mut _packet = [0u8; 256];
        // let packet = &mut _packet;

        let res = self.device.read(packet);
        let packet_len = match res {
            Ok(nfc::State::NewSession(x)) => {
                info!("State::NewSession");
                self.reset_state();
                x
            },
            Ok(nfc::State::Continue(x)) => x,
            Err(nfc::Error::NewSession) => {
                info!("Error::NewSession");
                self.reset_state();
                return Err(SourceError::NoActivity)
            },
            _ => {
                info!("nop");
                return Err(SourceError::NoActivity)
            }
        };


        assert!(packet_len > 0);

        // let packet = &self.packet;
        self.handle_block(&packet[.. packet_len as usize])?;

        info!(">>");
        info!("{}", hex_str!(&self.buffer));
        // logging::dump_hex(packet, l as usize);

        let command = command::Data::try_from_slice(&self.buffer);
        self.buffer.clear();
        if command.is_ok() {
            if self.interchange.request(
                command.unwrap()
            ).is_ok() {
                Ok(())
            } else {
                // Would be better to try canceling and taking on this apdu.
                info!("Had to drop most recent Apdu!");
                Err(SourceError::NoActivity)
            }
        } else {
            if let Some(last_iblock_recv) = self.last_iblock_recv {
                let (frame, _) = self.construct_iblock(
                    last_iblock_recv,
                    // UnspecifiedCheckingError
                    &[0x6F, 0x00]
                );

                self.send_frame( &frame )?;
            } else {
                info!("Session dropped.  This shouldn't happen.");
            }
            Err(SourceError::NoActivity)
        }
    }

    pub fn is_ready_to_transmit(&self) -> bool {
        self.interchange.state() == interchange::State::Responded
    }

    pub fn poll(&mut self) -> Iso14443Status {
        if interchange::State::Responded == self.interchange.state() {

            // important to wait on wtx reply from the reader.
            // If it wasn't sent, or we start replying before it's received,
            // then we could "double-send", which isn't permitted in iso14443-4.
            let mut wtx_wait_attempts = 0;
            while self.wtx_requested {
                info_now!("wait-for-wtx");
                let _did_recv_apdu = self.check_for_apdu();
                if !self.wtx_requested {
                    break;
                }
                wtx_wait_attempts += 1;
                if wtx_wait_attempts > 10 {
                    info_now!("no wtx reply, dumping the response.");
                    self.wtx_requested = false;
                    self.interchange.take_response();
                    return Iso14443Status::Idle;
                }
            }


            if let Some(msg) = self.interchange.take_response() {
                if let Some(last_iblock_recv) = self.last_iblock_recv {
                    info!("send!");
                    let (frame, data_used) = self.construct_iblock(last_iblock_recv, &msg);
                    self.send_frame(
                        &frame
                    ).ok();
                    if data_used != msg.len() {
                        info!("chaining response!");
                        self.buffer = msg;
                        self.state = Iso14443State::Transmitting(
                            0 .. data_used,
                            data_used .. self.buffer.len()
                        );
                    }
                } else {
                    info!("session was dropped! dropping response.");
                }
            }
            Iso14443Status::Idle
        } else {
            let did_recv_apdu = self.check_for_apdu();
            if did_recv_apdu.is_ok() {
                Iso14443Status::ReceivedData(Duration::from_millis(30))
            } else {
                Iso14443Status::Idle
            }
        }
    }

    pub fn poll_wait_extensions(&mut self) -> Iso14443Status {

        if self.wtx_requested {
            info!("warning: still awaiting wtx response.");
            return Iso14443Status::ReceivedData(Duration::from_millis(32));
        }

        match self.interchange.state() {
            interchange::State::Responded => {
                info!("could-send-from-wtx!");
                Iso14443Status::ReceivedData(Duration::from_millis(32))
            }
            interchange::State::Requested | interchange::State::Processing => {
                info!("send-wtx");
                self.send_wtx();
                self.wtx_requested = true;
                Iso14443Status::ReceivedData(Duration::from_millis(32))
            }
            _ => {
                info!("wtx done");
                Iso14443Status::Idle
            }
        }

    }

    /// Write response code + APDU
    fn send_frame(&mut self, buffer: &Iso14443Frame) -> Result<(), SourceError>
    {
        let r = self.device.send( buffer );
        if !r.is_ok() {
            return Err(SourceError::NoActivity);
        }

        info!("<<");
        if buffer.len() > 0 { info!("{}", hex_str!(&buffer)); }

        Ok(())
    }

}
