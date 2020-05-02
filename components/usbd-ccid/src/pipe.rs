use core::{
    borrow::{Borrow, BorrowMut},
    convert::{TryFrom, TryInto},
};

use cortex_m_semihosting::hprintln;

use crate::constants::*;

use usb_device::class_prelude::*;


pub struct Pipe<Bus>
where
    Bus: UsbBus + 'static,
{
    pub(crate) read: EndpointOut<'static, Bus>,
    pub(crate) write: EndpointIn<'static, Bus>,
    // pub(crate) rpc: TransportEndpoint<'rpc>,
    packet: Packet,
    buffer: [u8; MAX_MSG_LENGTH],
}

impl<Bus> Pipe<Bus>
where
    Bus: 'static + UsbBus,
{
    pub(crate) fn new(
        read: EndpointOut<'static, Bus>,
        write: EndpointIn<'static, Bus>,
    )
        -> Self { Self {
            read,
            write,
            packet: Packet { buffer: [0u8; PACKET_SIZE] },
            buffer: [0u8; MAX_MSG_LENGTH],
        } }
}

#[derive(Copy, Clone)]
pub struct Packet {
    buffer: [u8; PACKET_SIZE as _],
}

impl core::fmt::Debug for Packet {

    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_struct("Packet");
        let mut debug_struct = match self.msg_type() {
            Ok(message_type) => debug_struct.field("type", &message_type),
            Err(()) => debug_struct.field("type", &self[0]),
        };
        debug_struct
            .field("slot", &self.slot())
            .field("seq", &self.slot())
            // other fields?
            // .field("data[..16]", &self.data()[..16])
            // .field("data", self.data())
            .finish()
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum MessageType {
    PowerOn = 0x62,
    PowerOff = 0x63,
    GetSlotStatus = 0x65,
    TransferBlock = 0x6f,
}

pub struct SlotStatus {
    seq: u8
}

impl SlotStatus {
    pub fn new(seq: u8) -> Self {
        Self { seq }
    }
}

impl From<SlotStatus> for Packet {
    fn from(slot_status: SlotStatus) -> Self {
        let mut buffer = [0u8; PACKET_SIZE];
        buffer[0] = 0x81;
        // buffer[1..5] = 0, no extra data
        // buffer[5] = 0, only one slot
        buffer[6] = slot_status.seq;
        // buffer[7] = 0, status
        // buffer[8] = 0, error
        // buffer[9] = 0, chain parameter, this is complete
        Packet { buffer }
    }
}

pub struct DataBlock<'a> {
    seq: u8,
    data: &'a [u8],

}

impl<'a> DataBlock<'a> {
    pub fn new(seq: u8, data: &'a [u8]) -> Self {
        assert!(data.len() + 10 <= PACKET_SIZE);
        Self { seq, data }
    }
}

impl<'a> From<DataBlock<'a>> for Packet {
    fn from(data_block: DataBlock) -> Self {
        let mut buffer = [0u8; PACKET_SIZE];
        let len = data_block.data.len();
        buffer[0] = 0x80;
        buffer[1..5].copy_from_slice(&len.to_le_bytes());
        // buffer[5] = 0, only one slot
        buffer[6] = data_block.seq;
        // buffer[7] = 0, status
        // buffer[8] = 0, error
        // buffer[9] = 0, chain parameter, this is complete
        buffer[10..][..len].copy_from_slice(data_block.data);
        Packet { buffer }
    }
}

impl core::convert::TryFrom<u8> for MessageType {
    type Error = ();

    fn try_from(message_type_byte: u8) -> core::result::Result<Self, ()> {
        Ok(match message_type_byte {
            0x62 => Self::PowerOn,
            0x63 => Self::PowerOff,
            0x65 => Self::GetSlotStatus,
            0x6f => Self::TransferBlock,
            _ => return Err(()),
        })
    }
}

impl Packet {
    #[inline(always)]
    pub fn msg_type(&self) -> core::result::Result<MessageType, ()> {
        self[0].try_into()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        u32::from_le_bytes(self[1..5].try_into().unwrap()) as usize
    }

    #[inline(always)]
    pub fn slot(&self) -> u8 {
        // we have only one slot
        assert!(self[5] == 0);
        *&self[5]
    }

    #[inline(always)]
    pub fn seq(&self) -> u8 { *&self[6] }

    //  either three message specific bytes, or
    //  a status field (1 byte), an error field and one message specific byte

    #[inline(always)]
    pub fn data(&self) -> &[u8] { &self[10..] }
}

// impl core::borrow::Borrow<[u8; PACKET_SIZE]> for Packet {
//     fn borrow(&self) -> &[u8; PACKET_SIZE] {
//         &self.buffer
//     }
// }

// impl core::borrow::BorrowMut<[u8; PACKET_SIZE]> for Packet {
//     fn borrow_mut(&mut self) -> &mut [u8; PACKET_SIZE] {
//         &mut self.buffer
//     }
// }

impl core::ops::Deref for Packet {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        // let len = self.len();
        // &self.buffer[..10 + len]
        &self.buffer
    }
}

impl core::ops::DerefMut for Packet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // let len = self.len();
        // &mut self.buffer[..10 + len]
        &mut self.buffer
    }
}


impl<Bus> Pipe<Bus>
where
    Bus: 'static + UsbBus
{
    pub fn read_and_handle_packet(&mut self) {
        // let data: &mut [u8; PACKET_SIZE] = self.packet.borrow_mut();
        // let read = match self.read.read(&mut data[..]) {
        let read = match self.read.read(&mut self.packet) {
        // let read = match self.read.read(&mut self.packet.borrow_mut()[..]) {
            Ok(read) => {
                // all packets have 10 byte header before eventual data
                if read < 10 { panic!("unexpected small packet"); }
                read
            }
            Err(_) => {
                // usb-device lists WouldBlock + BufferOverflow as possible errors.
                // both should not occur here, and we can't do anything anyway.
                panic!("unexpected read error");
            }
        };

        hprintln!("got a packet of len {}: {:?}", read, &self.packet).ok();

        if let Ok(msg_type) = self.packet.msg_type() {
            match msg_type {
                MessageType::PowerOff |
                MessageType::GetSlotStatus => {
                    self.send_slot_status();
                }

                MessageType::PowerOn => {
                    self.send_atr();
                }

                MessageType::TransferBlock => {
                    //
                }
            }
        }
    }

    fn send_slot_status(&self) {
        let packet = Packet::from(SlotStatus::new(self.packet.seq()));
        hprintln!("answering with: {:?}", &packet).ok();
        match self.write.write(&packet[..10 + packet.len()]) {
            Ok(10) => {}

            Ok(n) => panic!("expected to send exactly 10 bytes, sent {}", n),
            Err(UsbError::WouldBlock) => panic!("would block not handled yet"),
            Err(e) => panic!("unexpected error {:?}", e),
        }
    }

    fn send_atr(&self) {
        let packet = Packet::from(DataBlock::new(
            self.packet.seq(),
            &[0x3b, 0x8c,0x80,0x01],
        ));
        hprintln!("answering with: {:?}", &packet).ok();

        match self.write.write(&packet[..10 + packet.len()]) {
            Ok(14) => {}

            Ok(n) => panic!("expected to send exactly 14 bytes, sent {}", n),
            Err(UsbError::WouldBlock) => panic!("would block not handled yet"),
            Err(e) => panic!("unexpected error {:?}", e),
        }
    }

    pub fn maybe_write_packet(&mut self) {
        // let result = self.write.write(&packet);

        // match result {
        //     Err(UsbError::WouldBlock) => {
        //         // fine, can't write try later
        //         // this shouldn't happen probably
        //     },
    }

    // pub fn read_address(&self) -> EndpointAddress {
    //     self.read.address()
    // }

    // pub fn write_address(&self) -> EndpointAddress {
    //     self.write.address()
    // }

}
