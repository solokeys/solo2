//! Wallet HID USB class types

use crate::dispatch::types::{Message, Requester, ResponseMessage, DEFAULT_MESSAGE_SIZE};
use crate::usbd::constants::{
    APDU_TAG, DEFAULT_CHANNEL, HID_DESCRIPTOR, HID_INTERFACE_CLASS, HID_REPORT_DESCRIPTOR,
    HID_REPORT_DESCRIPTOR_LENGTH, HID_REPORT_DESCRIPTOR_TYPE, INTERFACE_PROTOCOL_NONE,
    INTERFACE_SUBCLASS_NONE, INTERRUPT_POLL_MILLISECONDS, MESSAGE_SIZE, PACKET_SIZE,
};
use usb_device::{
    bus::{InterfaceNumber, UsbBus as UsbBusTrait, UsbBusAllocator},
    class::{ControlIn, ControlOut, UsbClass},
    control,
    descriptor::DescriptorWriter,
    endpoint::{EndpointAddress, EndpointIn, EndpointOut},
    Result as UsbResult,
};

/// State for receiving multi-packet messages
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
enum ReceiveState {
    Idle,
    Receiving {
        buffer: Message,
        sequence: u16,
        total_len: usize,
    },
}

/// State for sending multi-packet messages
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
enum SendState {
    Idle,
    Sending {
        buffer: ResponseMessage,
        offset: usize,
        sequence: u16,
    },
}

/// Wallet HID class implementation
pub struct WalletHid<'alloc, 'pipe, Bus: UsbBusTrait> {
    interface: InterfaceNumber,
    read_endpoint: EndpointOut<'alloc, Bus>,
    write_endpoint: EndpointIn<'alloc, Bus>,
    requester: Requester<'pipe, DEFAULT_MESSAGE_SIZE>,
    receive_state: ReceiveState,
    send_state: SendState,
    /// Ledger HID channel id from the last request; echoed back in responses.
    /// The host picks it (often random per session); the device must mirror it,
    /// not assume a fixed value. Defaults to `0x0101`.
    channel: [u8; 2],
}

impl<'alloc, 'pipe, Bus> WalletHid<'alloc, 'pipe, Bus>
where
    Bus: UsbBusTrait,
{
    pub fn new(
        allocate: &'alloc UsbBusAllocator<Bus>,
        requester: Requester<'pipe, DEFAULT_MESSAGE_SIZE>,
    ) -> Self {
        // 64 bytes, interrupt endpoint polled every 5 milliseconds
        let read_endpoint: EndpointOut<'alloc, Bus> =
            allocate.interrupt(PACKET_SIZE as u16, INTERRUPT_POLL_MILLISECONDS);
        // 64 bytes, interrupt endpoint polled every 5 milliseconds
        let write_endpoint: EndpointIn<'alloc, Bus> =
            allocate.interrupt(PACKET_SIZE as u16, INTERRUPT_POLL_MILLISECONDS);

        Self {
            interface: allocate.interface(),
            read_endpoint,
            write_endpoint,
            requester,
            receive_state: ReceiveState::Idle,
            send_state: SendState::Idle,
            channel: DEFAULT_CHANNEL,
        }
    }

    /// Read response from application (if any) and start writing it to the USB bus.
    /// Should be called before managing Bus.
    pub fn check_for_app_response(&mut self) {
        // This is called from UsbClasses::poll() to check for responses from the app
        // and send them via USB. The actual request processing happens in dispatch.poll()
        // which is called from the idle loop.
        self.handle_response();
        self.maybe_write_packet();
    }

    fn read_address(&self) -> EndpointAddress {
        self.read_endpoint.address()
    }

    fn write_address(&self) -> EndpointAddress {
        self.write_endpoint.address()
    }

    /// Handle incoming packet and assemble multi-packet messages
    fn read_and_handle_packet(&mut self) {
        let mut packet = [0u8; PACKET_SIZE];
        match self.read_endpoint.read(&mut packet) {
            Ok(PACKET_SIZE) => {}
            Ok(_size) => {
                // Unexpected size - ignore
                return;
            }
            Err(_error) => {
                // WouldBlock or other error - ignore
                return;
            }
        }

        // Ledger HID header: [channel_hi, channel_lo, tag(0x05), seq_hi, seq_lo, ...].
        // The channel is host-chosen (often random per session) and must be
        // echoed back, NOT required to be a fixed value — only the APDU tag is
        // checked here.
        if packet.len() < 5 || packet[2] != APDU_TAG {
            // Not an APDU-tagged packet - ignore
            return;
        }
        // Remember the channel so responses mirror it.
        self.channel = [packet[0], packet[1]];

        let seq = ((packet[3] as u16) << 8) | packet[4] as u16;

        if seq == 0 {
            // First packet - start new message
            if packet.len() < 7 {
                // Need at least 7 bytes for header + total_len
                return;
            }

            let total_len = ((packet[5] as usize) << 8) | packet[6] as usize;

            if total_len > MESSAGE_SIZE {
                // Message too large - reset and ignore
                self.receive_state = ReceiveState::Idle;
                return;
            }

            let mut buffer = Message::new();
            if packet.len() > 7 {
                let payload = &packet[7..];
                buffer.extend_from_slice(payload).ok();
            }

            if buffer.len() >= total_len {
                // Message fits in one packet
                buffer.truncate(total_len);
                if self.requester.request(buffer).is_err() {
                    // App is busy - drop the message and reset state
                    self.receive_state = ReceiveState::Idle;
                    return;
                }
                self.receive_state = ReceiveState::Idle;
            } else {
                // Need more packets
                self.receive_state = ReceiveState::Receiving {
                    buffer,
                    sequence: 1,
                    total_len,
                };
            }
        } else {
            // Continuation packet
            match &mut self.receive_state {
                ReceiveState::Receiving {
                    buffer,
                    sequence,
                    total_len,
                } => {
                    if seq != *sequence {
                        // Sequence mismatch - reset
                        self.receive_state = ReceiveState::Idle;
                        return;
                    }

                    if packet.len() > 5 {
                        let payload = &packet[5..];
                        buffer.extend_from_slice(payload).ok();
                    }

                    *sequence = seq + 1;

                    // Check if message is complete
                    if buffer.len() >= *total_len {
                        buffer.truncate(*total_len);
                        let complete_message = buffer.clone();
                        if self.requester.request(complete_message).is_err() {
                            // App is busy - drop the message and reset state
                            self.receive_state = ReceiveState::Idle;
                            return;
                        }
                        self.receive_state = ReceiveState::Idle;
                    }
                }
                ReceiveState::Idle => {
                    // Unexpected continuation packet - ignore
                }
            }
        }
    }

    // `Authenticator::call` always terminates the response with its own
    // ISO 7816 status word. Forward verbatim — appending another 0x9000
    // here mangles error returns (0x6985 → 0x6985 9000) into a fake-success
    // tail that the host CLI parses as "Approved" + malformed body.
    fn handle_response(&mut self) {
        if let SendState::Idle = self.send_state {
            if let Some(response) = self.requester.take_response() {
                let buffer = match response.0 {
                    Ok(data) => data,
                    Err(_) => {
                        let mut e = ResponseMessage::new();
                        e.push(0x6F).ok();
                        e.push(0x00).ok();
                        e
                    }
                };
                self.send_state = SendState::Sending {
                    buffer,
                    offset: 0,
                    sequence: 0,
                };
            }
        }
    }

    /// Send next packet if we have data to send
    fn maybe_write_packet(&mut self) {
        // Mirror the channel id the host used in its request.
        let channel = self.channel;
        match &mut self.send_state {
            SendState::Sending {
                buffer,
                offset,
                sequence,
            } => {
                let mut packet = [0u8; PACKET_SIZE];

                if *sequence == 0 {
                    // First packet: include total_len (2 bytes)
                    let total_len = buffer.len();
                    packet[0] = channel[0];
                    packet[1] = channel[1];
                    packet[2] = APDU_TAG;
                    packet[3] = (*sequence >> 8) as u8;
                    packet[4] = (*sequence & 0xFF) as u8;
                    packet[5] = (total_len >> 8) as u8;
                    packet[6] = (total_len & 0xFF) as u8;

                    let payload_start = 7;
                    let available_space = PACKET_SIZE - payload_start;
                    let remaining = buffer.len() - *offset;
                    let chunk_size = core::cmp::min(available_space, remaining);

                    packet[payload_start..payload_start + chunk_size]
                        .copy_from_slice(&buffer[*offset..*offset + chunk_size]);

                    match self.write_endpoint.write(&packet) {
                        Ok(PACKET_SIZE) => {
                            *offset += chunk_size;
                            *sequence += 1;

                            if *offset >= buffer.len() {
                                // Done sending
                                self.send_state = SendState::Idle;
                            }
                        }
                        Ok(_) => {
                            // Short write - unexpected
                        }
                        Err(_) => {
                            // WouldBlock or other error - try again later
                        }
                    }
                } else {
                    // Continuation packet
                    packet[0] = channel[0];
                    packet[1] = channel[1];
                    packet[2] = APDU_TAG;
                    packet[3] = (*sequence >> 8) as u8;
                    packet[4] = (*sequence & 0xFF) as u8;

                    let payload_start = 5;
                    let available_space = PACKET_SIZE - payload_start;
                    let remaining = buffer.len() - *offset;
                    let chunk_size = core::cmp::min(available_space, remaining);

                    packet[payload_start..payload_start + chunk_size]
                        .copy_from_slice(&buffer[*offset..*offset + chunk_size]);

                    match self.write_endpoint.write(&packet) {
                        Ok(PACKET_SIZE) => {
                            *offset += chunk_size;
                            *sequence += 1;

                            if *offset >= buffer.len() {
                                // Done sending
                                self.send_state = SendState::Idle;
                            }
                        }
                        Ok(_) => {
                            // Short write - unexpected
                        }
                        Err(_) => {
                            // WouldBlock or other error - try again later
                        }
                    }
                }
            }
            SendState::Idle => {
                // Nothing to send
            }
        }
    }
}

impl<'alloc, 'pipe, Bus> UsbClass<Bus> for WalletHid<'alloc, 'pipe, Bus>
where
    Bus: UsbBusTrait,
{
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> UsbResult<()> {
        writer.interface(
            self.interface,
            HID_INTERFACE_CLASS,
            INTERFACE_SUBCLASS_NONE,
            INTERFACE_PROTOCOL_NONE,
        )?;

        // HID descriptor
        writer.write(
            HID_DESCRIPTOR,
            &[
                0x11,                               // bLength
                0x01,                               // bcdHID (1.1)
                0x00,                               // bCountryCode (universal)
                0x01,                               // bNumDescriptors
                HID_REPORT_DESCRIPTOR_TYPE,         // bDescriptorType
                HID_REPORT_DESCRIPTOR_LENGTH as u8, // wDescriptorLength (low byte)
                0x00,                               // wDescriptorLength (high byte)
            ],
        )?;

        writer.endpoint(&self.read_endpoint)?;
        writer.endpoint(&self.write_endpoint)?;

        Ok(())
    }

    fn poll(&mut self) {
        self.handle_response();
        self.maybe_write_packet();
    }

    fn endpoint_out(&mut self, addr: EndpointAddress) {
        if addr == self.read_address() {
            self.read_and_handle_packet();
        }
    }

    fn endpoint_in_complete(&mut self, addr: EndpointAddress) {
        if addr == self.write_address() {
            self.maybe_write_packet();
        }
    }

    fn control_in(&mut self, xfer: ControlIn<Bus>) {
        let req = xfer.request();

        if req.request_type == control::RequestType::Standard
            && req.recipient == control::Recipient::Interface
            && req.index == u8::from(self.interface) as u16
        {
            // GetDescriptor (0x6) for HID report descriptor
            if req.request == control::Request::GET_DESCRIPTOR {
                xfer.accept(|data| {
                    assert!(data.len() >= HID_REPORT_DESCRIPTOR_LENGTH);
                    data[..HID_REPORT_DESCRIPTOR_LENGTH].copy_from_slice(&HID_REPORT_DESCRIPTOR);
                    Ok(HID_REPORT_DESCRIPTOR_LENGTH)
                })
                .ok();
            }
        }
    }

    fn control_out(&mut self, xfer: ControlOut<Bus>) {
        let req = xfer.request();

        if req.request_type == control::RequestType::Class
            && req.recipient == control::Recipient::Interface
            && req.index == u8::from(self.interface) as u16
        {
            // SetIdle (0xA) - tells device to NAK polls while report unchanged
            if req.request == 0xA {
                xfer.accept().ok();
            }
        }
    }
}
