use core::convert::TryFrom;

use apdu_dispatch::types::ContactInterchange;
use interchange::Requester;

use crate::{
    constants::*,
    types::{
        MessageBuffer,
        packet::{
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

pub struct Pipe<Bus>
where
    Bus: UsbBus + 'static,
{
    pub(crate) write: EndpointIn<'static, Bus>,
    // pub(crate) rpc: TransportEndpoint<'rpc>,
    seq: u8,
    state: State,
    // TODO: remove, use interchange
    message: MessageBuffer,
    interchange: Requester<ContactInterchange>,
    sent: usize,
    outbox: Option<RawPacket>,

    ext_packet: ExtPacket,
    packet_len: usize,
    receiving_long: bool,
    long_packet_missing: usize,
    in_chain: usize,
}

impl<Bus> Pipe<Bus>
where
    Bus: 'static + UsbBus,
{
    pub(crate) fn new(
        write: EndpointIn<'static, Bus>,
        request_pipe: Requester<ContactInterchange>,
    ) -> Self {

        assert!(MAX_MSG_LENGTH >= PACKET_SIZE);

        Self {
            write,
            seq: 0,
            state: State::Idle,
            sent: 0,
            outbox: None,
            message: MessageBuffer::new(),
            interchange: request_pipe,

            ext_packet: Default::default(),
            packet_len: 0,
            receiving_long: false,
            long_packet_missing: 0,
            in_chain: 0,
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
                // info_now!("pl {}, p {}, missing {}, in_chain {}", self.packet_len, packet.len(), self.long_packet_missing, self.in_chain).ok();
                // info_now!("packet: {:X?}", &self.ext_packet).ok();
                self.receiving_long = false;
            }
        }

        // info_now!("handle packet").ok();
        // info_now!("{:X?}", &packet).ok();
        // let p = packet.clone();
        // match PacketCommand::try_from(packet) {
        match PacketCommand::try_from(self.ext_packet.clone()) {
            Ok(command) => {
                self.seq = command.seq();
                info_now!(">> {:?}", &command);

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
                info_now!("unknown command {:X?}", &_p);
                self.seq = self.ext_packet[6];
                self.send_slot_status_error(Error::CommandNotSupported);
            }
        }
    }

    fn handle_transfer(&mut self, command: XfrBlock) {

        // state: Idle, Receiving, Processing, Sending,
        //
        // conts: BeginsAndEnds, Begins, Ends, Continues, ExpectDataBlock,

        // info_now!("handle xfrblock").ok();
        info_now!("{:X?}", &command);
        match self.state {

            State::Idle => {
                // invariant: BUFFER_SIZE >= PACKET_SIZE
                match command.chain() {
                    Chain::BeginsAndEnds => {
                        info_now!("begins and ends");
                        self.message.clear();
                        self.message.extend_from_slice(command.data()).unwrap();
                        self.call_app();
                        self.state = State::Processing;
                        // self.send_empty_datablock();
                    }
                    Chain::Begins => {
                        info_now!("begins");
                        self.message.clear();
                        self.message.extend_from_slice(command.data()).unwrap();
                        self.state = State::Receiving;
                        self.send_empty_datablock(Chain::ExpectingMore);
                    }
                    _ =>  panic!("unexpectedly in idle state"),
                }
            }

            State::Receiving => {
                match command.chain() {
                    Chain::Continues => {
                        info_now!("continues");
                        assert!(command.data().len() + self.message.len() <= MAX_MSG_LENGTH);
                        self.message.extend_from_slice(command.data()).unwrap();
                        self.send_empty_datablock(Chain::ExpectingMore);
                    }
                    Chain::Ends => {
                        info_now!("ends");
                        assert!(command.data().len() + self.message.len() <= MAX_MSG_LENGTH);
                        self.message.extend_from_slice(command.data()).unwrap();
                        self.call_app();
                        self.state = State::Processing;
                    }
                    _ =>  panic!("unexpectedly in receiving state"),
                }
            }

            State::Processing => {
                // info_now!("handle xfrblock").ok();
                // info_now!("{:X?}", &command).ok();
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

    fn call_app(&mut self) {
        info_now!("called piv app");
        let command = match iso7816::command::Data::try_from_slice(&self.message) {
            Ok(command) => command,
            Err(_) => {
                info_now!("could fit payload into Apdu buffer. Ignoring. {:?}", &self.message);
                return;
            }
        };
        self.interchange.request(command).expect("could not deposit command");
            // apdu::Command::try_from(&self.message).unwrap()
        // ).expect("could not deposit command");
        // info_now!("set ccid state to processing").ok();
        self.state = State::Processing;
        // todo!("have message of length {} to dispatch", self.message.len());
    }

    pub fn poll_app(&mut self) {
        // static mut i: usize = 0;
        // unsafe {
        //     if i < 100 {
        //         i += 1;
        //     } else {
        //         info_now!(".").ok();
        //     }
        // }
        if let State::Processing = self.state {
            // info_now!("processing, checking for response, interchange state {:?}",
            //           self.interchange.state()).ok();

            if let Some(message) = self.interchange.take_response() {
                self.message = message;

                // crate::piv::fake_piv(&mut self.message);

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
        // info_now!("priming {:?}", &primed_packet).ok();
        self.outbox = Some(primed_packet.into());

        // fast-lane response attempt
        self.maybe_send_packet();
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
        let packet = DataBlock::new(
            self.seq,
            Chain::BeginsAndEnds,

            // PivApplet just uses:
            // 3B 80 80 01 01
            //
            // 3B 88 80 01 80 57 53 6F 6C 6F 20 42 83
            // T=0, T=1, card issuer's data "Solo B"
            // https://smartcard-atr.apdu.fr/parse?ATR=3B+88+80+01+80+57+53+6F+6C+6F+20+42+83
            //
            // T=0, T=1, command chaining/extended Lc+Le/no logical channels, card issuer's data "Solo B"
            // 3B 8C 80 01 80 73 C0 21 C0 56 53 6F 6C 6F 20 42 D4
            // https://smartcard-atr.apdu.fr/parse?ATR=3B+8C+80+01+80+73+C0+21+C0+56+53+6F+6C+6F+20+42+D4
            b";\x8c\x80\x01\x80s\xc0!\xc0VSolo B\xd4"
            //
            // Not sure if we also need some TA/TB/TC data as in
            // https://smartcard-atr.apdu.fr/parse?ATR=3B+F8+13+00+00+81+31+FE+15+59+75+62+69+6B+65+79+34+D4
            // At least TB(1) is deprecated, so it makes no sense
            // Also, there TD(1) = 0x81 and TD(2) = 0x31 both refer to protocol T=1 which seems wrong

            // don't remember where i got this from
            // &[0x3b, 0x8c,0x80,0x01],
            // "corrected"?
            // &[
            //     // TS
            //     0x3b,
            //     // D1 follows, no historical bytes
            //     0x80,
            //     // nothing more, T = 0
            //     0x01,
            // ],
            // "simplified"?
            // &[
            //     // TS
            //     0x3b,
            //     // D1 follows, no historical bytes
            //     0x00,
            // ],
            // Yubikey FIDO+CCID
            // 3b:f8:13:00:00:81:31:fe:15:59:75:62:69:6b:65:79:34:d4
            // &[
            //     // TS
            //     0x3b,
            //     // TO = TA1, TB1, TB2, TB3 follow, 8 historical bytes
            //     0xf8,

            //     // TA1 = default clock (5MHz), default clock rate conversion (372)o
            //     // But sets Di to 3 instead of default of 1
            //     0x13,
            //     // TB1 deprecated, should not transmit
            //     0x00,
            //     // TC1 = "extra guard time", default of 0
            //     0x00,

            //     // TD1 = (Y2, T) -> follows D2, T = 1
            //     0x81,
            //     // TD2 = (Y2, T)
            //     0x31,
            //     // TA2
            //     0xfe,
            //     // TB2
            //     0x15,
            //     // T1 = first historical byte
            //     0x59,

            //     // "SoloBee"
            //     0x53, 0x6F, 0x6C, 0x6F, 0x42, 0x65 ,0x65,

            //     // Checksum
            //     0x94,
            // ],
            // Yubikey NEO OTP+U2F+CCID
            // 3b:fc:13:00:00:81:31:fe:15:59:75:62:69:6b:65:79:4e:45:4f:72:33:e1
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
                    //     info_now!("--> sent {:?}... successfully", &packet[..8]).ok();
                    // } else {
                    //     info_now!("--> sent {:?} successfully", packet).ok();
                    // }

                    if needs_zlp {
                        // info_now!("sending ZLP").ok();
                        self.outbox = Some(RawPacket::new());
                    } else {
                        self.outbox = None;
                    }

                }
                Ok(_) => panic!("short write"),

                Err(UsbError::WouldBlock) => {
                    // fine, can't write try later
                    // this shouldn't happen probably
                    info_now!("waiting to send");
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
        info_now!("ABORT expected for seq = {}", _seq);
        todo!();
    }

}
