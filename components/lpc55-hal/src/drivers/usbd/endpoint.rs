use core::{
    cmp::min,
};

#[cfg(not(feature = "nosync"))]
use core::marker::PhantomData;

use cortex_m::interrupt::{Mutex, CriticalSection};

use usb_device::{
    Result,
    UsbError,
    endpoint::EndpointType,
};

use crate::traits::usb::Usb;
use crate::typestates::init_state;

use super::{
    endpoint_memory::EndpointBuffer,
    endpoint_registers::Instance as EndpointRegistersInstance,
};

/// Arbitrates access to the endpoint-specific registers and packet buffer memory.
pub struct Endpoint  <USB>
where USB: Usb<init_state::Enabled> {
    out_buf: Option<Mutex<EndpointBuffer>>,
    setup_buf: Option<Mutex<EndpointBuffer>>,
    in_buf: Option<Mutex<EndpointBuffer>>,
    ep_type: Option<EndpointType>,
    index: u8,
    pub(crate) _marker: PhantomData<USB>,
}
unsafe impl<USB> Send for Endpoint<USB>
where USB: Usb<init_state::Enabled> + Send {}

impl<USB> Endpoint <USB>
where USB: Usb<init_state::Enabled> {
    pub fn new(index: u8) -> Endpoint<USB> {
        Endpoint::<USB> {
            out_buf: None,
            setup_buf: None,
            in_buf: None,
            ep_type: None,
            index,
            _marker: PhantomData,
        }
    }

    pub fn index(&self) -> u8 {self.index }

    pub fn ep_type(&self) -> Option<EndpointType> { self.ep_type }

    pub fn set_ep_type(&mut self, ep_type: EndpointType) { self.ep_type = Some(ep_type); }

    pub fn buf_addroff(&self, buf: &EndpointBuffer) -> u16 {
        // need to be 64 byte aligned
        debug_assert!(buf.addr() & ((1 << 6) - 1) == 0);
        // the bits above 21:6 are stored in databufstart
        (buf.addr() >> 6) as u16
    }

    // OUT
    pub fn is_out_buf_set(&self) -> bool { self.out_buf.is_some() }

    pub fn set_out_buf(&mut self, buffer: EndpointBuffer) {
        self.out_buf = Some(Mutex::new(buffer));
    }

    pub fn reset_out_buf(&self, cs: &CriticalSection, epl: &EndpointRegistersInstance) {
        // hardware modifies the NBytes and Offset entries, need to change them back periodically
        if !self.is_out_buf_set() { return; };

        let buf = self.out_buf.as_ref().unwrap().borrow(cs);
        let addroff = self.buf_addroff(buf);
        let len = buf.capacity() as u16;
        let i = self.index as usize;

        epl.eps[i].ep_out[0].modify(|_, w| w
            .nbytes::<USB>().bits(len)
            .addroff::<USB>().bits(addroff)
            .a().active()
            .d().enabled() // technically, marked as R (for reserved?) for EP0
            .s().not_stalled()
        );
    }

    // pub fn enable_out_interrupt(&self, usb: &USB1) {
    //     // usb.inten.modify(|r, w| unsafe { w.bits(r.bits() | ((1 << 10) - 1)) } );
    //     let i = self.index;
    //     usb.inten.modify(|r, w| unsafe { w.ep_int_en().bits(1 << (i << 1)) });
    // }

    // pub fn enable_in_interrupt(&self, usb: &USB1) {
    //     // usb.inten.modify(|r, w| unsafe { w.bits(r.bits() | ((1 << 10) - 1)) } );
    //     let i = self.index;
    //     usb.inten.modify(|r, w| unsafe { w.ep_int_en().bits(1 << ((i << 1) + 1)) });
    // }

    // SETUP
    pub fn is_setup_buf_set(&self) -> bool { self.setup_buf.is_some() }

    pub fn set_setup_buf(&mut self, buffer: EndpointBuffer) {
        self.setup_buf = Some(Mutex::new(buffer));
    }

    pub fn reset_setup_buf(&self, cs: &CriticalSection, epl: &EndpointRegistersInstance) {
        // I think this only has to be called once, as hardware never changes ADDROFF
        if !self.is_setup_buf_set() { return; };

        let buf = self.setup_buf.as_ref().unwrap().borrow(cs);
        let addroff = self.buf_addroff(buf);
        // SETUP is "second ep0out buffer" --> ep_out[1]
        epl.eps[0].ep_out[1].modify(|_, w| w.addroff::<USB>().bits(addroff));
    }

    // IN
    pub fn is_in_buf_set(&self) -> bool { self.in_buf.is_some() }

    pub fn set_in_buf(&mut self, buffer: EndpointBuffer) {
        self.in_buf = Some(Mutex::new(buffer));
    }

    pub fn reset_in_buf(&self, cs: &CriticalSection, epl: &EndpointRegistersInstance) {
        // hardware modifies the NBytes and Offset entries, need to change them back periodically

        if !self.is_in_buf_set() { return; };

        let buf = self.in_buf.as_ref().unwrap().borrow(cs);
        let addroff = self.buf_addroff(buf);
        // let len = buf.len() as u32;

        let i = self.index as usize;
        if i > 0 {
            debug_assert!(epl.eps[i].ep_in[0].read().a().is_not_active());
            // while epl.eps[i].ep_in[0].read().a().is_active() {}
        }

        if i == 0 {
            epl.eps[0].ep_in[0].modify(|_, w| w
                .nbytes::<USB>().bits(0)
                .addroff::<USB>().bits(addroff)
                .a().not_active()
                .s().not_stalled()
            );
        } else {
            epl.eps[i].ep_in[0].modify(|_, w| w
                .nbytes::<USB>().bits(0)
                .addroff::<USB>().bits(addroff)
                .d().enabled()
                .s().not_stalled()
            );
        }
    }

    pub fn configure(&self, cs: &CriticalSection, usb: &USB, epl: &EndpointRegistersInstance) {
        let ep_type = match self.ep_type {
            Some(t) => t,
            None => { return },
        };

        // no support for Isochronous endpoints
        debug_assert!(ep_type != EndpointType::Isochronous);

        // clear all the interrupts
        usb.intstat.write(|w| unsafe { w.bits(!0) } );
        debug_assert!(usb.intstat.read().bits() == 0);

        self.reset_out_buf(cs, epl);
        if self.index == 0 {
            self.reset_setup_buf(cs, epl);
        }
        self.reset_in_buf(cs, epl);
    }

    pub fn write(&self, buf: &[u8], cs: &CriticalSection, epl: &EndpointRegistersInstance) -> Result<usize> {
        if !self.is_in_buf_set() { return Err(UsbError::WouldBlock); }
        let in_buf = self.in_buf.as_ref().unwrap().borrow(cs);

        if buf.len() > in_buf.capacity() {
            return Err(UsbError::BufferOverflow);
        }

        let i = self.index as usize;

        if i == 0 {
            epl.eps[0].ep_in[0].modify(|_, w| w
                .a().not_active()
            );
            in_buf.write(buf);
            epl.eps[0].ep_in[0].modify(|_, w| w
                .nbytes::<USB>().bits(buf.len() as u16)
                .addroff::<USB>().bits(self.buf_addroff(in_buf))
                .s().not_stalled()
                .a().active()
            );
        } else {
            if epl.eps[i].ep_in[0].read().a().is_active() {
                // NB: With this test in place, `bench_bulk_read` from TestClass fails.
                // cortex_m_semihosting::hprintln!("can't write yet, EP {} IN still active", i).ok();
                //
                // NB: This test is need, otherwise e.g. in solo-bee get out-of-order packets
                return Err(UsbError::WouldBlock);
            }
            in_buf.write(buf);
            epl.eps[i].ep_in[0].modify(|_, w| w
                .nbytes::<USB>().bits(buf.len() as u16)
                .addroff::<USB>().bits(self.buf_addroff(in_buf))
                .d().enabled()
                .s().not_stalled()
                .a().active()
            );
        }

        Ok(buf.len())
    }

    pub fn read(&self, buf: &mut [u8], cs: &CriticalSection, usb: &USB, epl: &EndpointRegistersInstance) -> Result<usize> {

        if !self.is_out_buf_set() { return Err(UsbError::WouldBlock); }

        let i = self.index as usize;

        if i != 0 {
            // need an ergonomic way to map i to register field
            let ep_out_offset = i << 1;
            let ep_out_mask = 1u32 << ep_out_offset;

            let ep_out_int = (usb.intstat.read().bits() & ep_out_mask) != 0;

            let ep_out_is_active = epl.eps[i].ep_out[0].read().a().is_active();

            if ep_out_int && ep_out_is_active {
                // cortex_m_semihosting::hprintln!("what the hello, EP {} signals interrupt but it's still active", i).ok();
            }

            if !ep_out_int || ep_out_is_active {
                return Err(UsbError::WouldBlock);
            }
            let out_buf = self.out_buf.as_ref().unwrap().borrow(cs);

            let nbytes = epl.eps[i].ep_out[0].read().nbytes::<USB>().bits() as usize;

            // let count = min((out_buf.capacity() - nbytes) as usize, buf.len());
            let count = (out_buf.capacity() - nbytes) as usize;

            out_buf.read(&mut buf[..count]);

            unsafe { usb.intstat.write(|w| w.bits(ep_out_mask)) };

            // self.reset_out_buf(cs, epl);
            epl.eps[i].ep_out[0].modify(|_, w| w
                .nbytes::<USB>().bits(out_buf.capacity() as u16)
                .addroff::<USB>().bits(self.buf_addroff(out_buf))
                .a().active()
                // .d().enabled()
                // .s().not_stalled()
            );

            Ok(count)




        } else  {
            let intstat_r = usb.intstat.read();
            let devcmdstat_r = usb.devcmdstat.read();
            if !(intstat_r.ep0out().bit_is_set() || devcmdstat_r.setup().bit_is_set()) {
                return Err(UsbError::WouldBlock);
            }

            if devcmdstat_r.setup().bit_is_set() {
                if !self.is_setup_buf_set() { return Err(UsbError::WouldBlock); }

                let setup_buf = self.setup_buf.as_ref().unwrap().borrow(cs);
                if buf.len() < 8 {
                    // this should never occur
                    return Err(UsbError::BufferOverflow);
                }
                setup_buf.read(&mut buf[..8]);

                debug_assert!(usb.intstat.read().ep0out().bit_is_set());
                usb.intstat.write(|w| w.ep0out().set_bit());

                // UM insists: clear all these bits *before* clearing DEVCMDSTAT.SETUP
                epl.eps[0].ep_out[0].modify(|_, w| w.a().not_active().s().not_stalled());
                epl.eps[0].ep_in[0].modify(|_, w| w.a().not_active().s().not_stalled());

                usb.intstat.write(|w| w.ep0in().set_bit());
                debug_assert!(usb.intstat.read().ep0in().bit_is_clear());

                usb.devcmdstat.modify(|_, w| w.setup().set_bit());
                debug_assert!(usb.devcmdstat.read().setup().bit_is_clear());

                // prepare to receive more
                self.reset_out_buf(cs, epl);
                Ok(8)

            } else {
                let out_buf = self.out_buf.as_ref().unwrap().borrow(cs);
                let nbytes = epl.eps[0].ep_out[0].read().nbytes::<USB>().bits() as usize;
                let count = min((out_buf.capacity() - nbytes) as usize, buf.len());

                out_buf.read(&mut buf[..count]);

                self.reset_out_buf(cs, epl);
                usb.intstat.write(|w| w.ep0out().set_bit());

                Ok(count)
            }
        }
    }

}
