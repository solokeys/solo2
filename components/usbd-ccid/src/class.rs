use core::convert::TryFrom;

use cortex_m_semihosting::hprintln;
use interchange::Requester;

use crate::{
    constants::*,
    types::{
        ApduInterchange,
        ClassRequest,
        packet::{
            self,
            RawPacket,
        },
        tlv,
    },
    pipe::Pipe,
};

use usb_device::class_prelude::*;
type Result<T> = core::result::Result<T, UsbError>;

pub struct Ccid<Bus>
where
    Bus: 'static + UsbBus,
{
    interface_number: InterfaceNumber,
    read: EndpointOut<'static, Bus>,
    // interrupt: EndpointIn<'static, Bus>,
    pipe: Pipe<Bus>,
}

impl<Bus> Ccid<Bus>
where
    Bus: 'static + UsbBus,
{
    pub fn new(
        allocator: &'static UsbBusAllocator<Bus>,
        request_pipe: Requester<ApduInterchange>,
    ) -> Self {
        let read = allocator.bulk(PACKET_SIZE as _);
        let write = allocator.bulk(PACKET_SIZE as _);
        // TODO: Add interrupt endpoint, so PC/SC does not
        // constantly poll us with GetSlotStatus
        //
        // PROBLEM: We don't have enough endpoints on the peripheral :/
        // (USBHS should have one more)
        // let interrupt = allocator.interrupt(8 as _, 32);
        let pipe = Pipe::new(write, request_pipe);
        let interface_number = allocator.interface();
        Self { interface_number, read, /* interrupt, */ pipe }
    }

    // needs better name, maybe call directly
    pub fn sneaky_poll(&mut self) {
        self.poll();
    }
}

impl<Bus> UsbClass<Bus> for Ccid<Bus>
where
    Bus: 'static + UsbBus,
{
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter)
        -> Result<()>
    {
        writer.interface(
            self.interface_number,
            CLASS_CCID,
            SUBCLASS_NONE,
            TransferMode::Bulk as u8,
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

    fn poll(&mut self) {
        // hprintln!("poll of ccid").ok();
        self.pipe.poll_app();
        self.pipe.maybe_send_packet();
    }

    fn endpoint_in_complete(&mut self, addr: EndpointAddress) {
        if addr != self.pipe.write.address() { return; }

        self.pipe.maybe_send_packet();
    }

    fn endpoint_out(&mut self, addr: EndpointAddress) {
        if addr != self.read.address() { return; }

        let maybe_packet = RawPacket::try_from(
            |packet| self.read.read(packet));

        // should we return an error message
        // if the raw packet is invalid?
        if let Ok(packet) = maybe_packet {
            self.pipe.handle_packet(packet);
        }

    }

    fn control_in(&mut self, transfer: ControlIn<Bus>) {
        use usb_device::control::*;
        let Request { request_type, recipient, index, request, .. }
            = *transfer.request();
        if index as u8 != u8::from(self.interface_number) {
            return;
        }

        if (request_type, recipient) == (RequestType::Class, Recipient::Interface) {
            match ClassRequest::try_from(request) {
                Ok(request) => {
                    match request {
                        // not strictly needed, as our bNumClockSupported = 0
                        ClassRequest::GetClockFrequencies => {
                            transfer.accept(|data| {
                                data.copy_from_slice(&CLOCK_FREQUENCY_KHZ);
                                Ok(4)
                            }).ok();
                        },

                        // not strictly needed, as our bNumDataRatesSupported = 0
                        ClassRequest::GetDataRates => {
                            transfer.accept(|data| {
                                data.copy_from_slice(&DATA_RATE_BPS);
                                Ok(4)
                            }).ok();
                        },
                        _ => panic!("unexpected direction for {:?}", &request),
                    }
                }

                Err(()) => {
                    hprintln!("unexpected request: {}", request).ok();
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
                    hprintln!("unexpected request: {}", request).ok();
                }
            }
        }
    }

}
