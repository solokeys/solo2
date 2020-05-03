use core::convert::TryFrom;

use cortex_m_semihosting::hprintln;

use crate::{
    constants::*,
    types::*,
};

use usb_device::class_prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Receiving,
    Processing,
    ReadyToSend,
    Sending,
}

pub struct Pipe<Bus>
where
    Bus: UsbBus + 'static,
{
    pub(crate) write: EndpointIn<'static, Bus>,
    // pub(crate) rpc: TransportEndpoint<'rpc>,
    seq: u8,
    state: State,
    message: MessageBuffer,
    sent: usize,
    outbox: Option<RawPacket>,
}

impl<Bus> Pipe<Bus>
where
    Bus: 'static + UsbBus,
{
    pub(crate) fn new(write: EndpointIn<'static, Bus>) -> Self {

        assert!(MAX_MSG_LENGTH >= PACKET_SIZE);

        Self {
            write,
            seq: 0,
            state: State::Idle,
            sent: 0,
            outbox: None,
            message: MessageBuffer::new(),
        }
    }

    pub fn busy(&self) -> bool {
        // need more states, but if we're waiting
        // to send, we can't accept new packets
        self.outbox.is_some()
    }
}


impl<Bus> Pipe<Bus>
where
    Bus: 'static + UsbBus
{
    pub fn handle_packet(&mut self, packet: RawPacket) {

        match Command::try_from(packet) {
            Ok(command) => {
                self.seq = command.seq();
                hprintln!("{:?}", &command).ok();

                // happy path
                match command {
                    Command::PowerOn(_command) => self.send_atr(),

                    Command::PowerOff(_command) => self.send_slot_status_ok(),

                    Command::GetSlotStatus(_command) => self.send_slot_status_ok(),

                    Command::XfrBlock(command) => self.handle_transfer(command),

                    Command::Abort(_command) => {
                        todo!();
                    }
                }
            }

            Err(PacketError::ShortPacket) => {
                panic!("short packet!");
            }

            Err(PacketError::UnknownCommand(c)) => {
                panic!("unknown command byte 0x{:x}", c);
            }
        }
    }

    fn handle_transfer(&mut self, command: XfrBlock) {

        // state: Idle, Receiving, Processing, Sending,
        //
        // conts: BeginsAndEnds, Begins, Ends, Continues, ExpectDataBlock,

        match self.state {

            State::Idle => {
                // invariant: BUFFER_SIZE >= PACKET_SIZE
                match command.chain() {
                    Chain::BeginsAndEnds => {
                        self.message.clear();
                        self.message.extend_from_slice(command.data()).unwrap();
                        self.call_app();
                        self.state = State::Processing;
                        // self.send_empty_datablock();
                    }
                    Chain::Begins => {
                        self.message.clear();
                        self.message.extend_from_slice(command.data()).unwrap();
                        self.state = State::Receiving;
                        self.send_empty_datablock(Chain::ExpectingMore);
                    }
                    _ =>  panic!("{:?} unexpected in idle state"),
                }
            }

            State::Receiving => {
                match command.chain() {
                    Chain::Continues => {
                        assert!(command.data().len() + self.message.len() <= MAX_MSG_LENGTH);
                        self.message.extend_from_slice(command.data()).unwrap();
                        self.send_empty_datablock(Chain::ExpectingMore);
                    }
                    Chain::Ends => {
                        assert!(command.data().len() + self.message.len() <= MAX_MSG_LENGTH);
                        self.message.extend_from_slice(command.data()).unwrap();
                        self.call_app();
                        self.state = State::Processing;
                    }
                    _ =>  panic!("{:?} unexpected in receiving state"),
                }
            }

            State::Processing => {
                panic!("{:?} unexpected in processing state")
            }

            State::ReadyToSend => {
                panic!("{:?} unexpected in ready-to-send state")
            }

            State::Sending => {
                match command.chain() {
                    Chain::ExpectingMore => {
                        self.prime_outbox();
                    }
                    _ =>  panic!("{:?} unexpected in receiving state"),
                }
            }
        }
    }

    fn call_app(&mut self) {
        // todo!("have message of length {} to dispatch", self.message.len());
    }

    pub fn poll_app(&mut self) {
        self.fake_poll_app();
    }

    fn fake_poll_app(&mut self) {
        if let State::Processing = self.state {

            let apdu = Apdu::try_from(self.message.as_ref()).unwrap();
            hprintln!(":: {:?}", &apdu).ok();

            // we should have an open XfrBlock allowance
            self.state = State::ReadyToSend;
            // fake some data
            self.sent = 0;
            self.message.resize_default(128).ok();
            self.prime_outbox();
        }
    }

    pub fn prime_outbox(&mut self) {
        if self.state != State::ReadyToSend && self.state != State::Sending {
            return;
        }

        if self.outbox.is_some() { panic!(); }

        let chunk_size = core::cmp::min(PACKET_SIZE - 10, self.message.len() - self.sent);
        let chunk = &self.message[self.sent..][..chunk_size];
        self.sent += chunk_size;
        let more = self.sent < self.message.len();

        let chain = match (self.state, more) {
            (State::ReadyToSend, true) => { self.state = State::Sending; Chain::Begins }
            (State::ReadyToSend, false) => { self.state = State::Idle; Chain::BeginsAndEnds }
            (State::Sending, true) => Chain::Continues,
            (State::Sending, false) => { self.state = State::Idle; Chain::Ends }
            // logically impossible
            _ => { return; }
        };

        let primed_packet = DataBlock::new(self.seq, chain, chunk);
        hprintln!("priming {:?}", &primed_packet).ok();
        self.outbox = Some(primed_packet.into());

        // fast-lane response attempt
        self.maybe_send_packet();
    }

    fn send_empty_datablock(&mut self, chain: Chain) {
        let packet = DataBlock::new(self.seq, chain, &[]).into();
        self.send_packet_assuming_possible(packet);
    }

    fn send_slot_status_ok(&mut self)
                           // , icc_status: u8, command_status: u8, error: u8)
    {
        let mut packet = RawPacket::new();
        packet.resize_default(10).ok();
        packet[0] = 0x81;
        packet[6] = self.seq;
        self.send_packet_assuming_possible(packet);
    }

    fn send_atr(&mut self) {
        let packet = DataBlock::new(
            self.seq,
            Chain::BeginsAndEnds,
            &[0x3b, 0x8c,0x80,0x01],
        );
        self.send_packet_assuming_possible(packet.into());
    }


    fn send_packet_assuming_possible(&mut self, packet: RawPacket) {
        assert!(self.outbox.is_none());
        self.outbox = Some(packet);

        // fast-lane response attempt
        self.maybe_send_packet();
    }

    pub fn maybe_send_packet(&mut self) {
        if let Some(packet) = self.outbox.as_ref() {
            let needs_zlp = packet.len() == PACKET_SIZE;
            match self.write.write(packet) {
                Ok(n) if n == packet.len() => {
                    // if packet.len() > 8 {
                    //     hprintln!("--> sent {:?}... successfully", &packet[..8]).ok();
                    // } else {
                    //     hprintln!("--> sent {:?} successfully", packet).ok();
                    // }

                    if needs_zlp {
                        hprintln!("sending ZLP").ok();
                        self.outbox = Some(RawPacket::new());
                    } else {
                        self.outbox = None;
                    }

                }
                Ok(_) => panic!("short write"),

                Err(UsbError::WouldBlock) => {
                    // fine, can't write try later
                    // this shouldn't happen probably
                    hprintln!("waiting to send").ok();
                },

                Err(_) => panic!("unexpected send error"),
            }
        }
    }

    // pub fn read_address(&self) -> EndpointAddress {
    //     self.read.address()
    // }

    // pub fn write_address(&self) -> EndpointAddress {
    //     self.write.address()
    // }

    pub fn expect_abort(&mut self, slot: u8, seq: u8) {
        debug_assert!(slot == 0);
        hprintln!("ABORT expected for seq = {}", seq).ok();
        todo!();
    }

}
