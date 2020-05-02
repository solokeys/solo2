use core::convert::TryFrom;

use cortex_m_semihosting::hprintln;

use crate::{
    constants::*,
    pipe::Pipe,
};

use usb_device::class_prelude::*;

type Result<T> = core::result::Result<T, UsbError>;

pub struct Ccid<Bus>
where
    Bus: 'static + UsbBus,
{
    interface_number: InterfaceNumber,
    pipe: Pipe<Bus>,
    must_send_zlp: bool,
}

impl<Bus> Ccid<Bus>
where
    Bus: 'static + UsbBus,
{
    pub fn new(allocator: &'static UsbBusAllocator<Bus>) -> Self {
        let read_endpoint = allocator.bulk(PACKET_SIZE as _);
        let write_endpoint = allocator.bulk(PACKET_SIZE as _);
        let pipe = Pipe::new(read_endpoint, write_endpoint);
        let interface_number = allocator.interface();
        Self { interface_number, pipe, must_send_zlp: false }
    }
}

pub enum ClassRequests {
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
        writer.endpoint(&self.pipe.write);
        writer.endpoint(&self.pipe.read);
        Ok(())
    }

    fn poll(&mut self) {
        self.pipe.maybe_write_packet();
    }

    fn endpoint_in_complete(&mut self, addr: EndpointAddress) {
        if addr != self.pipe.write.address() { return; }

        if self.must_send_zlp {
            self.pipe.write.write(&[]).ok();
            self.must_send_zlp = false;
        } else {
            self.pipe.maybe_write_packet();
        }
    }

    fn endpoint_out(&mut self, addr: EndpointAddress) {
        if addr != self.pipe.read.address() { return; }

        self.pipe.read_and_handle_packet();
    }

    fn control_in(&mut self, transfer: ControlIn<Bus>) {
        use usb_device::control::*;

        let Request { request_type, recipient, index, request, .. } = *transfer.request();

        if index as u8 != u8::from(self.interface_number) {
            return;
        }

        if (request_type, recipient) == (RequestType::Class, Recipient::Interface) {
            match ClassRequest::try_from(request) {
                Ok(request) => {
                    match request {
                        ClassRequest::Abort => {
                            // oh yeah?
                            transfer.reject().ok();
                            todo!();
                        }

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
                    }
                }

                Err(()) => {
                    hprintln!("unexpected request: {}", request).ok();
                }
            }
        }
    }

    fn control_out(&mut self, transfer: ControlOut<Bus>) {
        // todo!();
    }

}
