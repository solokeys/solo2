/*!
The CTAP protocol is a series of atomic *transactions*, which consist
of a *request* message followed by a *response* message.

Messages may spread over multiple *packets*, starting with
an *initialization* packet, followed by zero or more *continuation* packets.

In the case of multiple clients, the first to get through its initialization
packet in device idle state locks the device for other channels (they will
receive busy errors).

No state is maintained between transactions.
*/

use core::convert::TryInto;
use core::convert::TryFrom;
// pub type ContactInterchange = usbd_ccid::types::ApduInterchange;
// pub type ContactlessInterchange = iso14443::types::ApduInterchange;

use ctaphid_dispatch::types::HidInterchange;
use ctaphid_dispatch::command::Command;

use ctap_types::{
    authenticator::Error as AuthenticatorError,
};



use interchange::Requester;

// use serde::Serialize;
use usb_device::{
    bus::{UsbBus},
    endpoint::{EndpointAddress, EndpointIn, EndpointOut},
    UsbError,
    // Result as UsbResult,
};

use crate::{
    constants::{
        // 7609
        MESSAGE_SIZE,
        // 64
        PACKET_SIZE,
    },
    types::KeepaliveStatus,
};

/// The actual payload of given length is dealt with separately
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub struct Request {
    channel: u32,
    command: Command,
    length: u16,
    timestamp: u32,
}

/// The actual payload of given length is dealt with separately
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub struct Response {
    channel: u32,
    command: Command,
    length: u16,
}

impl Response {
    pub fn from_request_and_size(request: Request, size: usize) -> Self {
        Self {
            channel: request.channel,
            command: request.command,
            length: size as u16,
        }
    }

    pub fn error_from_request(request: Request) -> Self {
        Self {
            channel: request.channel,
            command: ctaphid_dispatch::command::Command::Error,
            length: 1,
        }
    }
}

#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub struct MessageState {
    // sequence number of next continuation packet
    next_sequence: u8,
    // number of bytes of message payload transmitted so far
    transmitted: usize,
}

impl Default for MessageState {
    fn default() -> Self {
        Self {
            next_sequence: 0,
            transmitted: PACKET_SIZE - 7,
        }
    }
}

impl MessageState {
    // update state due to receiving a full new continuation packet
    pub fn absorb_packet(&mut self) {
        self.next_sequence += 1;
        self.transmitted += PACKET_SIZE - 5;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(unused)]
pub enum State {
    Idle,

    // if request payload data is larger than one packet
    Receiving((Request, MessageState)),

    // Processing(Request),

    // // the request message is ready, need to dispatch to authenticator
    // Dispatching((Request, Ctap2Request)),

    // waiting for response from authenticator
    WaitingOnAuthenticator(Request),

    WaitingToSend(Response),

    Sending((Response, MessageState)),
}

pub struct Pipe<'alloc, Bus: UsbBus> {

    read_endpoint: EndpointOut<'alloc, Bus>,
    write_endpoint: EndpointIn<'alloc, Bus>,
    state: State,

    interchange: Requester<HidInterchange>,

    // shared between requests and responses, due to size
    buffer: [u8; MESSAGE_SIZE],

    // we assign channel IDs one by one, this is the one last assigned
    // TODO: move into "app"
    last_channel: u32,

    // Indicator of implemented commands in INIT response.
    pub(crate) implements: u8,

    // timestamp that gets used for timing out CID's
    pub(crate) last_milliseconds: u32,

    // a "read once" indicator if now we're waiting on the application processing
    started_processing: bool,

    needs_keepalive: bool,
}

impl<'alloc, Bus: UsbBus> Pipe<'alloc, Bus> {

    // pub fn borrow_mut_authenticator(&mut self) -> &mut Authenticator {
    //     &mut self.authenticator
    // }

    pub(crate) fn new(
        read_endpoint: EndpointOut<'alloc, Bus>,
        write_endpoint: EndpointIn<'alloc, Bus>,
        interchange: Requester<HidInterchange>,
        initial_milliseconds: u32,
    ) -> Self
    {
        Self {
            read_endpoint,
            write_endpoint,
            state: State::Idle,
            interchange,
            buffer: [0u8; MESSAGE_SIZE],
            last_channel: 0,
            // Default to nothing implemented.
            implements: 0x80,
            last_milliseconds: initial_milliseconds,
            started_processing: false,
            needs_keepalive: false,
        }
    }

    pub fn read_address(&self) -> EndpointAddress {
        self.read_endpoint.address()
    }

    pub fn write_address(&self) -> EndpointAddress {
        self.write_endpoint.address()
    }

    // used to generate the configuration descriptors
    pub(crate) fn read_endpoint(&self) -> &EndpointOut<'alloc, Bus> {
        &self.read_endpoint
    }

    // used to generate the configuration descriptors
    pub(crate) fn write_endpoint(&self) -> &EndpointIn<'alloc, Bus> {
        &self.write_endpoint
    }

    fn cancel_ongoing_activity(&mut self) {
        // Remove response if it's there
        if let Some(_response) = self.interchange.take_response() {
        } else {
            // Cancel if there's a request or processing
            match self.interchange.state() {
                interchange::State::Requested |
                interchange::State::BuildingResponse => {
                    self.interchange.cancel().expect("canceled");
                }
                _ => {}
            }
        }

        self.state = State::Idle;
    }

    /// This method handles CTAP packets (64 bytes), until it has assembled
    /// a CTAP message, with which it then calls `dispatch_message`.
    ///
    /// During these calls, we can be in states: Idle, Receiving, Dispatching.
    pub(crate) fn read_and_handle_packet(&mut self) {
        // info_now!("got a packet!");
        let mut packet = [0u8; PACKET_SIZE];
        match self.read_endpoint.read(&mut packet) {
            Ok(PACKET_SIZE) => {},
            Ok(_size) => {
                // error handling?
                // from spec: "Packets are always fixed size (defined by the endpoint and
                // HID report descriptors) and although all bytes may not be needed in a
                // particular packet, the full size always has to be sent.
                // Unused bytes SHOULD be set to zero."
                // !("OK but size {}", size);
                info!("error unexpected size {}", _size);
                return;
            },
            // usb-device lists WouldBlock or BufferOverflow as possible errors.
            // both should not occur here, and we can't do anything anyway.
            // Err(UsbError::WouldBlock) => { return; },
            // Err(UsbError::BufferOverflow) => { return; },
            Err(_error) => {
                info!("error no {}", _error as i32);
                return;
            },
        };
        info!(">> ");
        info!("{}", hex_str!(&packet[..16]));

        // packet is 64 bytes, reading 4 will not panic
        let channel = u32::from_be_bytes(packet[..4].try_into().unwrap());
        // info_now!("channel {}", channel);

        let is_initialization = (packet[4] >> 7) != 0;
        // info_now!("is_initialization {}", is_initialization);

        if is_initialization {
            // case of initialization packet
            info!("init");

            let command_number = packet[4] & !0x80;
            // info_now!("command number {}", command_number);

            let command = match Command::try_from(command_number) {
                Ok(command) => command,
                // `solo ls` crashes here as it uses command 0x86
                Err(_) => {
                    info!("Ignoring invalid command.");
                    return; },
            };

            // can't actually fail
            let length = u16::from_be_bytes(packet[5..][..2].try_into().unwrap());

            let timestamp = self.last_milliseconds;
            let current_request = Request { channel, command, length, timestamp};

            if !(self.state == State::Idle) {
                let request = match self.state {
                    State::WaitingOnAuthenticator(request) => {
                        request
                    },
                    State::Receiving((request, _message_state)) => {
                        request
                    },
                    _ => {
                        info!("Ignoring transaction as we're already transmitting.");
                        return;
                    },
                };
                if packet[4] == 0x86 {
                    info!("Resyncing!");
                    self.cancel_ongoing_activity();
                } else {
                    if channel == request.channel {
                        info!("Expected seq");
                        self.start_sending_error(request, AuthenticatorError::InvalidSeq);
                    } else {
                        info!("busy.");
                        self.send_error_now(current_request, AuthenticatorError::ChannelBusy);
                    }

                    return;
                }
            }


            if length > MESSAGE_SIZE as u16 {
                info!("Error message too big.");
                self.send_error_now(current_request, AuthenticatorError::InvalidLength);
                return;
            }

            if length > PACKET_SIZE as u16 - 7 {
                // store received part of payload,
                // prepare for continuation packets
                self.buffer[..PACKET_SIZE - 7]
                    .copy_from_slice(&packet[7..]);
                self.state = State::Receiving((current_request, {
                    let state = MessageState::default();
                    // info_now!("got {} so far", state.transmitted);
                    state
                }));
                // we're done... wait for next packet
                return;
            } else {
                // request fits in one packet
                self.buffer[..length as usize]
                    .copy_from_slice(&packet[7..][..length as usize]);
                self.dispatch_request(current_request);
                return;
            }
        } else {
            // case of continuation packet
            match self.state {
                State::Receiving((request, mut message_state)) => {
                    let sequence = packet[4];
                    // info_now!("receiving continuation packet {}", sequence);
                    if sequence != message_state.next_sequence {
                        // error handling?
                        // info_now!("wrong sequence for continuation packet, expected {} received {}",
                        //           message_state.next_sequence, sequence);
                        info!("Error invalid cont pkt");
                        self.start_sending_error(request, AuthenticatorError::InvalidSeq);
                        return;
                    }
                    if channel != request.channel {
                        // error handling?
                        // info_now!("wrong channel for continuation packet, expected {} received {}",
                        //           request.channel, channel);
                        info!("Ignore invalid channel");
                        return;
                    }

                    let payload_length = request.length as usize;
                    if message_state.transmitted + (PACKET_SIZE - 5) < payload_length {
                        // info_now!("transmitted {} + (PACKET_SIZE - 5) < {}",
                        //           message_state.transmitted, payload_length);
                        // store received part of payload
                        self.buffer[message_state.transmitted..][..PACKET_SIZE - 5]
                            .copy_from_slice(&packet[5..]);
                        message_state.absorb_packet();
                        self.state = State::Receiving((request, message_state));
                        // info_now!("absorbed packet, awaiting next");
                        return;
                    } else {
                        let missing = request.length as usize - message_state.transmitted;
                        self.buffer[message_state.transmitted..payload_length]
                            .copy_from_slice(&packet[5..][..missing]);
                        self.dispatch_request(request);
                    }
                },
                _ => {
                    // unexpected continuation packet
                    info!("Ignore unexpected cont pkt");
                    return;
                },
            }
        }
    }

    pub fn check_timeout(&mut self, milliseconds: u32) {
        // At any point the RP application could crash or something,
        // so its up to the device to timeout those transactions.
        let last = self.last_milliseconds;
        self.last_milliseconds = milliseconds;
        match &mut self.state {
            State::Receiving((request, _message_state)) => {
                if (milliseconds - last) > 200 {
                    // If there's a lapse in `check_timeout(...)` getting called (e.g. due to logging),
                    // this could lead to inaccurate timestamps on requests.  So we'll
                    // just "forgive" requests temporarily if this happens.
                    debug!("lapse in hid check.. {} {} {}", request.timestamp, milliseconds, last);
                    request.timestamp = milliseconds;
                }
                // compare keeping in mind of possible overflow in timestamp.
                else if (milliseconds > request.timestamp && (milliseconds - request.timestamp) > 550)
                || (milliseconds < request.timestamp && milliseconds > 550)
                {
                    debug!("Channel timeout. {}, {}, {}", request.timestamp, milliseconds, last);
                    let req = *request;
                    self.start_sending_error(req, AuthenticatorError::Timeout);
                }
            }
            _ => { }
        };
    }

    fn dispatch_request(&mut self, request: Request) {

        match request.command {
            Command::Init => {}
            _ => {
                if request.channel == 0xffffffff {
                    self.start_sending_error(request, AuthenticatorError::InvalidChannel);
                    return;
                }
            }
        }
        // dispatch request further
        match request.command {
            Command::Init => {
                // info_now!("command INIT!");
                // info_now!("data: {:?}", &self.buffer[..request.length as usize]);
                match request.channel {
                    0 => {
                        // this is an error / reserved number
                        self.start_sending_error(request, AuthenticatorError::InvalidChannel);
                    },

                    // broadcast channel ID - request for assignment
                    cid => {
                        if request.length != 8 {
                            // error
                            info!("Invalid length for init.  ignore.");
                        } else {
                            self.last_channel += 1;
                            // info_now!(
                            //     "assigned channel {}", self.last_channel);
                            let _nonce = &self.buffer[..8];
                            let response = Response {
                                channel: cid,
                                command: request.command,
                                length: 17,
                            };

                            self.buffer[8..12].copy_from_slice(&self.last_channel.to_be_bytes());
                            // CTAPHID protocol version
                            self.buffer[12] = 2;
                            // major device version number
                            self.buffer[13] = 0;
                            // minor device version number
                            self.buffer[14] = 0;
                            // build device version number
                            self.buffer[15] = 0;
                            // capabilities flags
                            // 0x1: implements WINK
                            // 0x4: implements CBOR
                            // 0x8: does not implement MSG
                            // self.buffer[16] = 0x01 | 0x08;
                            self.buffer[16] = self.implements;
                            self.start_sending(response);
                        }
                    },
                }
            },

            Command::Ping => {
                let response = Response::from_request_and_size(request, request.length as usize);
                self.start_sending(response);
            },

            _ => {
                if request.command == Command::Cbor {
                    self.needs_keepalive = true;
                } else {
                    self.needs_keepalive = false;
                }
                if self.interchange.state() == interchange::State::Responded {
                    info!("dumping stale response");
                    self.interchange.take_response();
                }
                match self.interchange.request(
                    &(request.command, heapless::Vec::from_slice(&self.buffer[..request.length as usize]).unwrap())
                ) {
                    Ok(_) => {
                        self.state = State::WaitingOnAuthenticator(request);
                        self.started_processing = true;
                    },
                    Err(_) => {
                        // busy
                        info_now!("STATE: {:?}", self.interchange.state());
                        info!("can't handle more than one authenticator request at a time.");
                        self.send_error_now(request, AuthenticatorError::ChannelBusy);
                    }
                }
            },
        }
    }

    pub fn did_start_processing(&mut self) -> bool{
        if self.started_processing {
            self.started_processing = false;
            true
        } else {
            false
        }
    }

    pub fn send_keepalive(&mut self, is_waiting_for_user_presence: bool) -> bool {
        if let State::WaitingOnAuthenticator(request) = &self.state {
            if !self.needs_keepalive {
                // let response go out normally in idle loop
                info!("cmd does not need keepalive messages");
                false
            } else {

                info!("keepalive");

                let mut packet = [0u8; PACKET_SIZE];

                packet[..4].copy_from_slice(&request.channel.to_be_bytes());
                packet[4] = 0x80 | 0x3B;
                packet[5..7].copy_from_slice(&1u16.to_be_bytes());

                if is_waiting_for_user_presence {
                    packet[7] = KeepaliveStatus::UpNeeded as u8;
                } else {
                    packet[7] = KeepaliveStatus::Processing as u8;
                }

                self.write_endpoint.write(&packet).ok();

                true
            }
        } else {
            info!("keepalive done");
            false
        }
    }

    pub fn handle_response(&mut self) {
        if let State::WaitingOnAuthenticator(request) = self.state {


            if let Some(response) = self.interchange.take_response() {
                match response {

                    Err(ctaphid_dispatch::app::Error::InvalidCommand) => {
                        info!("Got waiting reply from authenticator??");
                        self.start_sending_error(request, AuthenticatorError::InvalidCommand);

                    }
                    Err(ctaphid_dispatch::app::Error::InvalidLength) => {
                        info!("Error, payload needed app command.");
                        self.start_sending_error(request, AuthenticatorError::InvalidLength);
                    }
                    Err(ctaphid_dispatch::app::Error::NoResponse) => {
                        info!("Got waiting noresponse from authenticator??");
                    }

                    Ok(message) => {
                        info!("Got {} bytes response from authenticator, starting send", message.len());
                        let response = Response::from_request_and_size(request, message.len());
                        self.buffer[..message.len()]
                            .copy_from_slice(&message);
                        self.start_sending(response);
                    }
                }
            }
        }

    }

    fn start_sending(&mut self, response: Response) {
        self.state = State::WaitingToSend(response);
        self.maybe_write_packet();
    }

    fn start_sending_error(&mut self, request: Request, error: AuthenticatorError){
        self.buffer[0] = error as u8;
        let response = Response::error_from_request(request);
        self.start_sending(response);
    }

    fn send_error_now(&mut self, request: Request, error: AuthenticatorError){
        let last_state = core::mem::replace(&mut self.state, State::Idle);
        let last_first_byte = self.buffer[0];

        self.buffer[0] = error as u8;
        let response = Response::error_from_request(request);
        self.start_sending(response);
        self.maybe_write_packet();

        self.state = last_state;
        self.buffer[0] = last_first_byte;
    }

    // called from poll, and when a packet has been sent
    pub(crate) fn maybe_write_packet(&mut self) {

        match self.state {
            State::WaitingToSend(response) => {

                // zeros leftover bytes
                let mut packet = [0u8; PACKET_SIZE];
                packet[..4].copy_from_slice(&response.channel.to_be_bytes());
                // packet[4] = response.command.into() | 0x80u8;
                packet[4] = response.command.into_u8() | 0x80;
                packet[5..7].copy_from_slice(&response.length.to_be_bytes());

                let fits_in_one_packet = 7 + response.length as usize <= PACKET_SIZE;
                if fits_in_one_packet {
                    packet[7..][..response.length as usize]
                        .copy_from_slice( &self.buffer[..response.length as usize]);
                    self.state = State::Idle;
                } else {
                    packet[7..].copy_from_slice(&self.buffer[..PACKET_SIZE - 7]);
                }

                // try actually sending
                // info_now!("attempting to write init packet {:?}, {:?}",
                //           &packet[..32], &packet[32..]);
                let result = self.write_endpoint.write(&packet);

                match result {
                    Err(UsbError::WouldBlock) => {
                        // fine, can't write try later
                        // this shouldn't happen probably
                        info!("hid usb WouldBlock");
                    },
                    Err(_) => {
                        // info_now!("weird USB errrorrr");
                        panic!("unexpected error writing packet!");
                    },
                    Ok(PACKET_SIZE) => {
                        // goodie, this worked
                        if fits_in_one_packet {
                            self.state = State::Idle;
                            // info_now!("StartSent {} bytes, idle again", response.length);
                            // info_now!("IDLE again");
                        } else {
                            self.state = State::Sending((response, MessageState::default()));
                            // info_now!(
                            //     "StartSent {} of {} bytes, waiting to send again",
                            //     PACKET_SIZE - 7, response.length);
                            // info_now!("State: {:?}", &self.state);
                        }
                    },
                    Ok(_) => {
                        // info_now!("short write");
                        panic!("unexpected size writing packet!");
                    },
                };
            },

            State::Sending((response, mut message_state)) => {
                // info_now!("in StillSending");
                let mut packet = [0u8; PACKET_SIZE];
                packet[..4].copy_from_slice(&response.channel.to_be_bytes());
                packet[4] = message_state.next_sequence;

                let sent = message_state.transmitted;
                let remaining = response.length as usize - sent;
                let last_packet = 5 + remaining <= PACKET_SIZE;
                if last_packet {
                    packet[5..][..remaining].copy_from_slice(
                        &self.buffer[message_state.transmitted..][..remaining]);
                } else {
                    packet[5..].copy_from_slice(
                        &self.buffer[message_state.transmitted..][..PACKET_SIZE - 5]);
                }

                // try actually sending
                // info_now!("attempting to write cont packet {:?}, {:?}",
                //           &packet[..32], &packet[32..]);
                let result = self.write_endpoint.write(&packet);

                match result {
                    Err(UsbError::WouldBlock) => {
                        // fine, can't write try later
                        // this shouldn't happen probably
                        // info_now!("can't send seq {}, write endpoint busy",
                        //           message_state.next_sequence);
                    },
                    Err(_) => {
                        // info_now!("weird USB error");
                        panic!("unexpected error writing packet!");
                    },
                    Ok(PACKET_SIZE) => {
                        // goodie, this worked
                        if last_packet {
                            self.state = State::Idle;
                            // info_now!("in IDLE state after {:?}", &message_state);
                        } else {
                            message_state.absorb_packet();
                            // DANGER! destructuring in the match arm copies out
                            // message state, so need to update state
                            // info_now!("sent one more, now {:?}", &message_state);
                            self.state = State::Sending((response, message_state));
                        }
                    },
                    Ok(_) => {
                        debug!("short write");
                        panic!("unexpected size writing packet!");
                    },
                };
            },

            // nothing to send
            _ => {
            },
        }
    }
}


