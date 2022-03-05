use core::convert::TryFrom;

use embedded_time::duration::Extensions;
use heapless::Vec;
use interchange::{Interchange, Requester};

use crate::{
    constants::*,
    types::{
        ClassRequest,
        packet::RawPacket,
        Status,
    },
    pipe::Pipe,
};

use usb_device::class_prelude::*;
type Result<T> = core::result::Result<T, UsbError>;

pub struct Ccid<Bus, I, const N: usize>
where
    Bus: 'static + UsbBus,
    I: 'static + Interchange<REQUEST = Vec<u8, N>, RESPONSE = Vec<u8, N>>,
{
    interface_number: InterfaceNumber,
    string_index: StringIndex,
    read: EndpointOut<'static, Bus>,
    // interrupt: EndpointIn<'static, Bus>,
    pipe: Pipe<Bus, I, N>,
}

impl<Bus, I, const N: usize> Ccid<Bus, I, N>
where
    Bus: 'static + UsbBus,
    I: 'static + Interchange<REQUEST = Vec<u8, N>, RESPONSE = Vec<u8, N>>,
{
    /// Class constructor.
    ///
    /// The optional card issuer's data may be of length at most 13 bytes,
    /// and allows personalizing the Answer-to-Reset, for instance by
    /// ASCII-encoding vendor or model information.
    pub fn new(
        allocator: &'static UsbBusAllocator<Bus>,
        request_pipe: Requester<I>,
        card_issuers_data: Option<&[u8]>,
    ) -> Self {
        let read = allocator.bulk(PACKET_SIZE as _);
        let write = allocator.bulk(PACKET_SIZE as _);
        // TODO: Add interrupt endpoint, so PC/SC does not
        // constantly poll us with GetSlotStatus
        //
        // PROBLEM: We don't have enough endpoints on the peripheral :/
        // (USBHS should have one more)
        // let interrupt = allocator.interrupt(8 as _, 32);
        let pipe = Pipe::new(write, request_pipe, card_issuers_data);
        let interface_number = allocator.interface();
        let string_index = allocator.string();
        Self { interface_number, string_index, read, /* interrupt, */ pipe }
    }

    /// Read response from application (if any) and start writing it to
    /// the USB bus.  Should be called before managing Bus.
    pub fn check_for_app_response(&mut self) {
        self.poll();
    }

    pub fn did_start_processing(&mut self) -> Status {
        if self.pipe.did_started_processing() {
            // We should send a wait extension later
            Status::ReceivedData(1_000.milliseconds())
        } else {
            Status::Idle
        }
    }

    pub fn send_wait_extension (&mut self) -> Status {
        if self.pipe.send_wait_extension() {
            // We should send another wait extension later
            Status::ReceivedData(1_000.milliseconds())
        } else {
            Status::Idle
        }
    }
}

impl<Bus, I, const N: usize> UsbClass<Bus> for Ccid<Bus, I, N>
where
    Bus: 'static + UsbBus,
    I: 'static + Interchange<REQUEST = Vec<u8, N>, RESPONSE = Vec<u8, N>>,
{
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter)
        -> Result<()>
    {
        writer.interface_alt(
            self.interface_number,
            0,
            CLASS_CCID,
            SUBCLASS_NONE,
            TransferMode::Bulk as u8,
            Some(self.string_index),
        )?;
        writer.write(
            FUNCTIONAL_INTERFACE,
            &FUNCTIONAL_INTERFACE_DESCRIPTOR,
        )?;
        writer.endpoint(&self.pipe.write).unwrap();
        writer.endpoint(&self.read).unwrap();
        // writer.endpoint(&self.interrupt).unwrap();
        Ok(())
    }

    fn get_string(&self, index: StringIndex, _lang_id: u16) -> Option<&str> {
        (self.string_index == index)
            .then(|| FUNCTIONAL_INTERFACE_STRING)
    }

    #[inline(never)]
    fn poll(&mut self) {
        // info_now!("poll of ccid");
        self.pipe.poll_app();
        self.pipe.maybe_send_packet();
    }

    fn endpoint_in_complete(&mut self, addr: EndpointAddress) {
        if addr != self.pipe.write.address() { return; }

        self.pipe.maybe_send_packet();
    }

    fn endpoint_out(&mut self, addr: EndpointAddress) {
        if addr != self.read.address() { return; }

        // let maybe_packet = RawPacket::try_from(
        //     |packet| self.read.read(packet));

        let maybe_packet = {
            let mut packet = RawPacket::new();
            packet.resize_default(packet.capacity()).unwrap();
            let result = self.read.read(&mut packet);
            result.map(|count| {
                packet.resize_default(count).unwrap();
                packet
            })
        };

        // should we return an error message
        // if the raw packet is invalid?
        if let Ok(packet) = maybe_packet {
            self.pipe.handle_packet(packet);
        }

    }

    fn control_in(&mut self, transfer: ControlIn<Bus>) {
        use usb_device::control::*;
        let Request { request_type, recipient, index, request, .. } = *transfer.request();
        if index != u8::from(self.interface_number) as u16 {
            return;
        }

        if (request_type, recipient) == (RequestType::Class, Recipient::Interface) {
            match ClassRequest::try_from(request) {
                Ok(request) => {
                    match request {
                        // not strictly needed, as our bNumClockSupported = 0
                        ClassRequest::GetClockFrequencies => {
                            transfer.accept_with(&CLOCK_FREQUENCY_KHZ).ok();
                        },

                        // not strictly needed, as our bNumDataRatesSupported = 0
                        ClassRequest::GetDataRates => {
                            transfer.accept_with_static(&DATA_RATE_BPS).ok();
                        },
                        _ => panic!("unexpected direction for {:?}", &request),
                    }
                }

                Err(()) => {
                    info_now!("unexpected request: {}", request);
                    transfer.reject().ok();
                }
            }
        }
    }

    fn control_out(&mut self, transfer: ControlOut<Bus>) {
        use usb_device::control::*;
        let Request { request_type, recipient, index, request, value, .. }
            = *transfer.request();
        if index as u8 != u8::from(self.interface_number) {
            return;
        }

        if (request_type, recipient) == (RequestType::Class, Recipient::Interface) {
            match ClassRequest::try_from(request) {
                Ok(request) => {
                    match request {
                        ClassRequest::Abort => {
                            // spec: "slot in low, seq in high byte"
                            let [slot, seq] = value.to_le_bytes();
                            self.pipe.expect_abort(slot, seq);
                            transfer.accept().ok();

                            // // old behaviour
                            // transfer.reject().ok();
                            // todo!();
                        }
                        _ => panic!("unexpected direction for {:?}", &request),
                    }
                }

                Err(()) => {
                    info_now!("unexpected request: {}", request);
                }
            }
        }
    }

}
