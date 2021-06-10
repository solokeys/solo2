use core::convert::TryFrom;

use heapless_bytes::Bytes;
use interchange::{Interchange, Requester};

use crate::{
    constants::*,
    types::packet::{
        Chain,
        Command as PacketCommand,
        DataBlock,
        Error as PacketError,
        ExtPacket,
        RawPacket,
        XfrBlock,

        ChainedPacket as _,
        PacketWithData as _,
    },
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum Error {
    CmdAborted = 0xff,
    IccMute = 0xfe,
    XfrParityError = 0xfd,
    //..
    CmdSlotBusy = 0xE0,
    CommandNotSupported = 0x00,
}

pub struct Pipe<Bus, I, N>
where
    Bus: 'static + UsbBus,
    I: 'static + Interchange<REQUEST = Bytes<N>, RESPONSE = Bytes<N>>,
    N: heapless::ArrayLength<u8>,
{
    pub(crate) write: EndpointIn<'static, Bus>,
    // pub(crate) rpc: TransportEndpoint<'rpc>,
    seq: u8,
    state: State,
    interchange: Requester<I>,
    sent: usize,
    outbox: Option<RawPacket>,

    ext_packet: ExtPacket,
    #[allow(dead_code)]
    packet_len: usize,
    receiving_long: bool,
    long_packet_missing: usize,
    in_chain: usize,
    pub(crate) started_processing: bool,
    atr: Bytes<heapless::consts::U32>,
}

impl<Bus, I, N> Pipe<Bus, I, N>
where
    Bus: 'static + UsbBus,
    I: 'static + Interchange<REQUEST = Bytes<N>, RESPONSE = Bytes<N>>,
    N: heapless::ArrayLength<u8>,
{
    pub(crate) fn new(
        write: EndpointIn<'static, Bus>,
        request_pipe: Requester<I>,
        card_issuers_data: Option<&[u8]>,
    ) -> Self {

        assert!(MAX_MSG_LENGTH >= PACKET_SIZE);

        Self {
            write,
            seq: 0,
            state: State::Idle,
            sent: 0,
            outbox: None,
            interchange: request_pipe,

            ext_packet: Default::default(),
            packet_len: 0,
            receiving_long: false,
            long_packet_missing: 0,
            in_chain: 0,
            started_processing: false,
            // later on, we only signal T=1 support
            // if for some reason not signaling T=0 support leads to issues,
            // we can enable it here.
            atr: Self::construct_atr(card_issuers_data, false),
        }
    }

    fn construct_atr(card_issuers_data: Option<&[u8]>, signal_t_equals_0: bool) -> Bytes<heapless::consts::U32> {
        assert!(card_issuers_data.map_or(true, |data| data.len() <= 13));
        let k = card_issuers_data.map_or(0u8, |data| 2 + data.len() as u8);
        let mut atr = Bytes::new();
        // TS: direct convention
        atr.push(0x3B).ok();
        // T0: encode length of historical bytes
        atr.push(0x80 | k).ok();
        if signal_t_equals_0 {
            // T=0, more to follow
            atr.push(0x80).ok();
        }
        // T=1
        atr.push(0x01).ok();

        if let Some(data) = card_issuers_data {
            // no status indicator
            atr.push(0x80).ok();
            // tag 5: card issuer's data
            atr.push(0x50 | data.len() as u8).ok();
            atr.extend_from_slice(data).ok();
        }
        // xor of all bytes except TS
        let mut checksum = 0;
        for byte in atr.iter().skip(1) {
            checksum ^= *byte;
        }
        atr.push(checksum).ok();

        atr
    }

    pub fn busy(&self) -> bool {
        // need more states, but if we're waiting
        // to send, we can't accept new packets
        self.outbox.is_some()
    }
}


impl<Bus, I, N> Pipe<Bus, I, N>
where
    Bus: 'static + UsbBus,
    I: 'static + Interchange<REQUEST = Bytes<N>, RESPONSE = Bytes<N>>,
    N: heapless::ArrayLength<u8>,
{
    pub fn handle_packet(&mut self, packet: RawPacket) {
        use crate::types::packet::RawPacketExt;

        // SHOULD CLEAN THIS UP!
        // The situation is as follows: full 64B USB packet received.
        // CCID packet signals no command chaining, but data length > 64 - 10.
        // Then we can expect to receive more USB packets containing only data.
        // The concatenation of all these is then a valid Command APDU.
        // (which itself may have command chaining on a higher level, e.g.
        // when certificates are transmitted, because PIV somehow uses short APDUs
        // only (can we fix this), so 255B is the maximum)
        if !self.receiving_long {
            if packet.len() < 10 {
                panic!("unexpected short packet");
            }
            self.ext_packet.clear();
            // TODO check
            self.ext_packet.extend_from_slice(&packet).unwrap();

            let pl = packet.packet_len();
            if pl > 54 {
                self.receiving_long = true;
                self.in_chain = 1;
                self.long_packet_missing = pl - 54;
                self.packet_len = pl;
                return;
            } else {
                // normal case
            }
        } else {
            // TODO check
            self.ext_packet.extend_from_slice(&packet).ok();
            self.in_chain += 1;
            assert!(packet.len() <= self.long_packet_missing);
            self.long_packet_missing -= packet.len();
            if self.long_packet_missing > 0 {
                return;
            } else {
                // info!("pl {}, p {}, missing {}, in_chain {}", self.packet_len, packet.len(), self.long_packet_missing, self.in_chain).ok();
                // info!("packet: {:X?}", &self.ext_packet).ok();
                self.receiving_long = false;
            }
        }

        // info!("{:X?}", &packet).ok();
        // let p = packet.clone();
        // match PacketCommand::try_from(packet) {
        match PacketCommand::try_from(self.ext_packet.clone()) {
            Ok(command) => {
                self.seq = command.seq();

                // happy path
                match command {
                    PacketCommand::PowerOn(_command) => self.send_atr(),

                    PacketCommand::PowerOff(_command) => self.send_slot_status_ok(),

                    PacketCommand::GetSlotStatus(_command) => self.send_slot_status_ok(),

                    PacketCommand::XfrBlock(command) => self.handle_transfer(command),

                    PacketCommand::Abort(_command) => {
                        todo!();
                    }
                    PacketCommand::GetParameters(_command) => self.send_parameters(),
                }
            }

            Err(PacketError::ShortPacket) => {
                panic!("short packet!");
            }

            Err(PacketError::UnknownCommand(_p)) => {
                info!("unknown command {:X?}", &_p);
                self.seq = self.ext_packet[6];
                self.send_slot_status_error(Error::CommandNotSupported);
            }
        }
    }

    #[inline(never)]
    fn reset_interchange(&mut self) {
        let message = Bytes::new();
        self.interchange.take_response();
        // this may no longer be needed
        // before the interchange change (adding the request_mut method),
        // one necessary side-effect of this was to set the interchange's
        // enum variant to Request.
        self.interchange.request(&message).ok();
        self.interchange.cancel().ok();
    }

    fn handle_transfer(&mut self, command: XfrBlock) {

        // state: Idle, Receiving, Processing, Sending,
        //
        // conts: BeginsAndEnds, Begins, Ends, Continues, ExpectDataBlock,

        // info!("handle xfrblock").ok();
        // info!("{:X?}", &command);
        match self.state {

            State::Idle => {
                // invariant: BUFFER_SIZE >= PACKET_SIZE
                match command.chain() {
                    Chain::BeginsAndEnds => {
                        info!("begins and ends");
                        self.reset_interchange();
                        let message = self.interchange.request_mut().unwrap();
                        message.clear();
                        message.extend_from_slice(command.data()).unwrap();
                        self.call_app();
                        self.state = State::Processing;
                        // self.send_empty_datablock();
                    }
                    Chain::Begins => {
                        info!("begins");
                        self.reset_interchange();
                        let message = self.interchange.request_mut().unwrap();
                        message.clear();
                        message.extend_from_slice(command.data()).unwrap();
                        self.state = State::Receiving;
                        self.send_empty_datablock(Chain::ExpectingMore);
                    }
                    _ =>  panic!("unexpectedly in idle state"),
                }
            }

            State::Receiving => {
                match command.chain() {
                    Chain::Continues => {
                        info!("continues");
                        let message = self.interchange.request_mut().unwrap();
                        assert!(command.data().len() + message.len() <= MAX_MSG_LENGTH);
                        message.extend_from_slice(command.data()).unwrap();
                        self.send_empty_datablock(Chain::ExpectingMore);
                    }
                    Chain::Ends => {
                        info!("ends");
                        let message = self.interchange.request_mut().unwrap();
                        assert!(command.data().len() + message.len() <= MAX_MSG_LENGTH);
                        message.extend_from_slice(command.data()).unwrap();
                        self.call_app();
                        self.state = State::Processing;
                    }
                    _ =>  panic!("unexpectedly in receiving state"),
                }
            }

            State::Processing => {
                // info!("handle xfrblock").ok();
                // info!("{:X?}", &command).ok();
                panic!("ccid pipe unexpectedly received command while in processing state: {:?}", &command);
            }

            State::ReadyToSend => {
                panic!("unexpectedly in ready-to-send state")
            }

            State::Sending => {
                match command.chain() {
                    Chain::ExpectingMore => {
                        self.prime_outbox();
                    }
                    _ =>  panic!("unexpectedly in receiving state"),
                }
            }
        }
    }

    pub fn send_wait_extension(&mut self) -> bool {
        if self.state == State::Processing {
            // Need to send a wait extension request.
            let mut packet = RawPacket::new();
            packet.resize_default(10).ok();
            packet[0] = 0x80;
            packet[6] = self.seq;

            // CCID_Rev110 6.2-3: Time Extension is requested
            packet[7] = 2 << 6;
            // Perhaps 1 is an ok multiplier?
            packet[8] = 0x1;
            self.send_packet_assuming_possible(packet);

            // Indicate we should check back again for another possible wait extension
            true
        } else {
            // No longer processing, so the reply has been sent, and we no longer need more time.
            false
        }
    }

    /// Turns false on read.  Intended for checking to see if a wait extension request needs to be started.
    pub fn did_started_processing(&mut self) -> bool {
        if self.started_processing {
            self.started_processing = false;
            true
        } else {
            false
        }
    }

    #[inline(never)]
    fn call_app(&mut self) {
        self.interchange.send_request().expect("could not deposit command");
        self.started_processing = true;
        self.state = State::Processing;
    }

    pub fn poll_app(&mut self) {
        if let State::Processing = self.state {
            // info!("processing, checking for response, interchange state {:?}",
            //           self.interchange.state()).ok();

            if interchange::State::Responded == self.interchange.state() {

                // we should have an open XfrBlock allowance
                self.state = State::ReadyToSend;
                self.sent = 0;
                self.prime_outbox();
            }
        }
    }

    pub fn prime_outbox(&mut self) {
        if self.state != State::ReadyToSend && self.state != State::Sending {
            return;
        }

        if self.outbox.is_some() { panic!(); }

        // if let Some(message) = self.interchange.response() {
            let message: &mut Bytes<N> = unsafe { self.interchange.interchange.rp_mut() };

            let chunk_size = core::cmp::min(PACKET_SIZE - 10, message.len() - self.sent);
            let chunk = &message[self.sent..][..chunk_size];
            self.sent += chunk_size;
            let more = self.sent < message.len();

            let chain = match (self.state, more) {
                (State::ReadyToSend, true) => { self.state = State::Sending; Chain::Begins }
                (State::ReadyToSend, false) => { self.state = State::Idle; Chain::BeginsAndEnds }
                (State::Sending, true) => Chain::Continues,
                (State::Sending, false) => { self.state = State::Idle; Chain::Ends }
                // logically impossible
                _ => { return; }
            };

            let primed_packet = DataBlock::new(self.seq, chain, chunk);
            // info!("priming {:?}", &primed_packet).ok();
            self.outbox = Some(primed_packet.into());

            // fast-lane response attempt
            self.maybe_send_packet();
        // }
    }

    fn send_empty_datablock(&mut self, chain: Chain) {
        let packet = DataBlock::new(self.seq, chain, &[]).into();
        self.send_packet_assuming_possible(packet);
    }

    fn send_slot_status_ok(&mut self) {
        let mut packet = RawPacket::new();
        packet.resize_default(10).ok();
        packet[0] = 0x81;
        packet[6] = self.seq;
        self.send_packet_assuming_possible(packet);
    }

    fn send_slot_status_error(&mut self, error: Error) {
        let mut packet = RawPacket::new();
        packet.resize_default(10).ok();
        packet[0] = 0x6c;
        packet[6] = self.seq;
        packet[7] = 1<<6;
        packet[8] = error as u8;
        self.send_packet_assuming_possible(packet);
    }

    fn send_parameters(&mut self) {
        let mut packet = RawPacket::new();
        packet.resize_default(17).ok();
        packet[0] = 0x82;
        packet[1] = 7;
        packet[6] = self.seq;
        packet[9] = 1; // T=1

        // just picking the fastest values.
        //              Fi = 1Mz    Di=1
        packet[10] = (0b0001 << 4) | (0b0001);

        // just taking default value from spec.
        packet[11] = 0x10;
        // not sure, taking default.
        packet[13] = 0x15;
        // set max waiting time
        packet[15] = 0xfe;
        self.send_packet_assuming_possible(packet);
    }

    fn send_atr(&mut self) {
        let atr = self.atr.clone();
        let packet = DataBlock::new(
            self.seq,
            Chain::BeginsAndEnds,
            &atr,

            // T=0, T=1, command chaining/extended Lc+Le/no logical channels, card issuer's data "Solo 2"
            // 3B 8C 80 01 80 73 C0 21 C0 56 53 6F 6C 6F 20 32 A4
            // https://smartcard-atr.apdu.fr/parse?ATR=3B+8C+80+01+80+73+C0+21+C0+56+53+6F+6C+6F+20+32+A4
            // &[0x3B, 0x8C, 0x80, 0x01, 0x80, 0x73, 0xC0, 0x21, 0xC0, 0x56, 0x53, 0x6F, 0x6C, 0x6F, 0x20, 0x32, 0xA4]
            //
            // Not sure if we also need some TA/TB/TC data as in
            // https://smartcard-atr.apdu.fr/parse?ATR=3B+F8+13+00+00+81+31+FE+15+59+75+62+69+6B+65+79+34+D4
            // At least TB(1) is deprecated, so it makes no sense
            // Also, there TD(1) = 0x81 and TD(2) = 0x31 both refer to protocol T=1 which seems wrong
        );
        self.send_packet_assuming_possible(packet.into());
    }


    fn send_packet_assuming_possible(&mut self, packet: RawPacket) {
        if !self.outbox.is_none() {
            // Previous transaction will fail, but we'll be ready for new transactions.
            self.state = State::Idle;
            info!("overwriting last session..");
        }
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
                    //     info!("--> sent {:?}... successfully", &packet[..8]).ok();
                    // } else {
                    //     info!("--> sent {:?} successfully", packet).ok();
                    // }

                    if needs_zlp {
                        self.outbox = Some(RawPacket::new());
                    } else {
                        self.outbox = None;
                    }

                }
                Ok(_) => panic!("short write"),

                Err(UsbError::WouldBlock) => {
                    // fine, can't write try later
                    // this shouldn't happen probably
                    info!("waiting to send");
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

    pub fn expect_abort(&mut self, slot: u8, _seq: u8) {
        debug_assert!(slot == 0);
        info!("ABORT expected for seq = {}", _seq);
        todo!();
    }

}
