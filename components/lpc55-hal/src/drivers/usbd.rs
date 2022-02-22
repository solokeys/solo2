pub mod prelude {
    pub use super::UsbBus;
    pub use super::UsbError as UsbError;
    pub use super::Result as UsbResult;
}

pub mod constants;

pub mod endpoint;
use endpoint::Endpoint;

pub mod endpoint_memory;
use endpoint_memory::{
    EndpointMemoryAllocator,
};

pub mod endpoint_registers;

// move this into a submodule `bus` again?

use core::mem;

use cortex_m::interrupt::{
    self,
    Mutex,
};

pub use usb_device::{
    Result,
    UsbError,
};

use usb_device::{
    UsbDirection,
    endpoint::{
        EndpointType,
        EndpointAddress,
    },
    bus::{
        UsbBusAllocator,
        PollResult,
    },
};

use crate::traits::usb::{
    Usb,
};
use crate::{
    typestates::{
        init_state,
    }
};
use crate::{
    Pin,
    drivers::pins::PinId,
    typestates::pin,
};

pub trait Usb0VbusPin: Send { }
impl<P> Usb0VbusPin for Pin<P, pin::state::Special<pin::function::USB0_VBUS>> where P: PinId + Send {}

/// Implements the `usb_device::bus::UsbBus` trait.
///
/// From that documentation:
///
/// The UsbBus is shared by reference between the global UsbDevice as well as UsbClasses,
/// and therefore any required mutability must be implemented using interior mutability.
/// Most operations that may mutate the bus object itself take place before enable is called.
/// After the bus is enabled, in practice most access won't mutate the object itself
/// but only endpoint-specific registers and buffers, the access to which is mostly
/// arbitrated by endpoint handles.
///
/// Warning: Currently, `UsbBus` uses the same `USB1_SRAM` memory region for both FS and HS
/// peripherals, so it's not possible to use both peripherals at the same time with this driver.
pub struct UsbBus<USB>
where
    USB: Usb<init_state::Enabled> + Send,
{
    usb_regs: Mutex<USB>,
    ep_regs: Mutex<endpoint_registers::Instance>,
    endpoints: [Endpoint<USB>; self::constants::NUM_ENDPOINTS],
    ep_allocator: EndpointMemoryAllocator,
    max_endpoint: usize,
}


impl<USB> UsbBus<USB>
where
    USB: Usb<init_state::Enabled> + Send,
{
    /// Constructs a new USB peripheral driver.
    pub fn new<PIN>(usb_device: USB, _usb0_vbus_pin: PIN) -> UsbBusAllocator<UsbBus<USB>>
        where PIN: Usb0VbusPin + Sync
    {
        use self::constants::NUM_ENDPOINTS;

        let bus = UsbBus {
            usb_regs: Mutex::new(usb_device),
            ep_regs: Mutex::new(endpoint_registers::attach().unwrap()),
            ep_allocator: EndpointMemoryAllocator::new(),
            max_endpoint: 0,
            endpoints: {
                let mut endpoints: [mem::MaybeUninit<Endpoint<USB>>; NUM_ENDPOINTS] = unsafe {
                    mem::MaybeUninit::uninit().assume_init()
                };

                for (i, endpoint) in endpoints.iter_mut().enumerate() {
                    *endpoint = mem::MaybeUninit::new(Endpoint::<USB>::new(i as u8));
                }

                unsafe { mem::transmute::<_, [Endpoint<USB>; NUM_ENDPOINTS]>(endpoints) }
            },
        };

        UsbBusAllocator::new(bus)
    }

    pub fn clear_interrupt(&mut self) {
        // clear interrupt, otherwise in an interrupt-driven setting
        // like RTIC the idle loop will get starved.
        interrupt::free(|cs| {
            // set device address to 0
            let usb = self.usb_regs.borrow(cs);
            usb.intstat.write(|w| w.dev_int().set_bit());
        });
    }


}


// impl<PINS: Send+Sync> usb_device::bus::UsbBus for UsbBus<PINS> {
impl<USB> usb_device::bus::UsbBus for UsbBus<USB>
where
    USB: Usb<init_state::Enabled> + Send,
{

    // override the default (contrary to USB spec),
    // as describe in the user manual
    const QUIRK_SET_ADDRESS_BEFORE_STATUS: bool = true;

    fn alloc_ep(
        &mut self,
        ep_dir: UsbDirection,
        ep_addr: Option<EndpointAddress>,
        ep_type: EndpointType,
        max_packet_size: u16,
        _interval: u8) -> Result<EndpointAddress>
    {
        // well this is clever but is it readable
        for index in ep_addr.map(|a| a.index()..a.index() + 1).unwrap_or(1..self::constants::NUM_ENDPOINTS) {
            let ep = &mut self.endpoints[index];

            match ep.ep_type() {
                None => { ep.set_ep_type(ep_type); },
                Some(t) if t != ep_type => { continue; },
                _ => { },
            };

            match ep_dir {
                UsbDirection::Out if !ep.is_out_buf_set() => {
                    let size = max_packet_size;
                    let buffer = self.ep_allocator.allocate_buffer(size as _)?;
                    ep.set_out_buf(buffer);
                    debug_assert!(ep.is_out_buf_set());

                    if index == 0 {
                        let setup = self.ep_allocator.allocate_buffer(8)?;
                        ep.set_setup_buf(setup);
                    }

                    return Ok(EndpointAddress::from_parts(index, ep_dir));
                },

                UsbDirection::In if !ep.is_in_buf_set() => {
                    let size = max_packet_size;
                    let buffer = self.ep_allocator.allocate_buffer(size as _)?;
                    ep.set_in_buf(buffer);

                    return Ok(EndpointAddress::from_parts(index, ep_dir));
                },

                _ => { }
            }
        }

        Err(match ep_addr {
            Some(_) => UsbError::InvalidEndpoint,
            None => UsbError::EndpointOverflow,
        })
    }

    fn enable(&mut self) {
        // cortex_m_semihosting::hprintln!("Enabling UsbBus").ok();
        interrupt::free(|cs| {
            let usb = self.usb_regs.borrow(cs);
            let eps = self.ep_regs.borrow(cs);

            let mut max = 0;
            for (index, ep) in self.endpoints.iter().enumerate() {
                if ep.is_out_buf_set() || ep.is_in_buf_set() {
                    max = index;

                    // not sure this is needed
                    if ep.is_out_buf_set() {
                        ep.reset_out_buf(cs, eps);
                        if index == 0 { ep.reset_setup_buf(cs, eps); }
                        // ep.enable_out_interrupt(usb);
                    }
                    if ep.is_in_buf_set() {
                        ep.reset_in_buf(cs, eps);
                        // ep.enable_in_interrupt(usb);
                    }
                }
            }
            self.max_endpoint = max;

            // DATABUFSTART
            unsafe {
                // lower part is stored in endpoint registers
                let databufstart = constants::EP_MEM_ADDR as u32;
                usb.databufstart.modify(|_, w| w.da_buf().bits(databufstart));
            };

            // EPLISTSTART
            unsafe {
                let epliststart = eps.addr;
                debug_assert!(epliststart as u8 == 0); // needs to be 256 byte aligned
                usb.epliststart.modify(|_, w| w.ep_list().bits(epliststart >> 8));
            }

            // ENABLE + CONNECT
            usb.devcmdstat.modify(|_, w| w.dev_en().set_bit().dcon().set_bit());

            // HERE TOO?
            usb.inten.modify(|r, w| unsafe { w.bits(r.bits() | ((1 << 12) - 1)) } );
            usb.inten.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 31)) } );
        });
    }

    fn reset(&self) {
        // cortex_m_semihosting::hprintln!("Resetting UsbBus").ok();
        interrupt::free(|cs| {
            // set device address to 0
            let usb = self.usb_regs.borrow(cs);
            let eps = self.ep_regs.borrow(cs);

            usb.devcmdstat.modify(|_, w| unsafe { w.dev_addr().bits(0) } );

            for ep in self.endpoints.iter() {
                ep.configure(cs, usb, eps);
            }

            // clear all interrupts
            usb.intstat.write(|w| unsafe { w.bits(!0) } );

            // enable them
            // TODO: do this by endpoint
            // cortex_m_semihosting::hprintln!("inten bef = {:#X}", usb.inten.read().bits()).ok();
            // usb.inten.modify(|r, w| unsafe { w.bits(r.bits() | ((1 << 10) - 1)) } );
            // usb.inten.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 31)) } );
            // cortex_m_semihosting::hprintln!("inten aft = {:#X}", usb.inten.read().bits()).ok();
        });
        // #[cfg(feature = "logging")]
        // info!("reset USB device").ok();
    }

    fn set_device_address(&self, addr: u8) {
        // cortex_m_semihosting::hprintln!("Setting UsbBus device address {}", addr).ok();
        interrupt::free(|cs| {
            self.usb_regs.borrow(cs).devcmdstat.modify(|_, w| unsafe {
                w.dev_addr().bits(addr)
            });
        });
    }

    fn poll(&self) -> PollResult {
        interrupt::free(|cs| {
            let usb = self.usb_regs.borrow(cs);
            let eps = self.ep_regs.borrow(cs);
            // WOAH WHYY DOES IT WORK WITH THIS??
            // cortex_m_semihosting::hprintln!("inten = {:#X}", usb.inten.read().bits()).ok();
            // let _ = usb.inten.read().bits();

            let devcmdstat = &usb.devcmdstat;
            let intstat = &usb.intstat;

            // let ints = intstat.read().bits();
            // if ints != 0 {
            //     cortex_m_semihosting::hprintln!("intstat = {:?}", intstat.read().bits()).ok();
            //     cortex_m_semihosting::hprintln!("inten = {:?}", usb.inten.read().bits()).ok();
            // }

            // if intstat.read().dev_int().bit_is_set() {
            //     usb.intstat.write(|w| w.dev_int().set_bit());
            // }

            // Bus reset flag?
            if devcmdstat.read().dres_c().bit_is_set() {
                devcmdstat.modify(|_, w| w.dres_c().set_bit());
                // debug_assert!(devcmdstat.read().dres_c().bit_is_clear());
                return PollResult::Reset
            }

            // if devcmdstat.read().dsus_c().bit_is_set() {
            //     cortex_m_semihosting::hprintln!("suspend bit set!").ok();
            // }

            // if devcmdstat.read().dsus_c().bit_is_set() {
            //     cortex_m_semihosting::hprintln!("device suspended!").ok();
            // }

            // if devcmdstat.read().dcon_c().bit_is_set() {
            //     cortex_m_semihosting::hprintln!("connect bit set!").ok();
            // }

            // TODO: Resume, Suspend handling

            let mut ep_out = 0;
            let mut ep_in_complete = 0;
            let mut ep_setup = 0;

            let mut bit = 1;

            // NB: these are not "reader objects", but the actual value
            // of the registers at time of assignment :))
            let intstat_r = intstat.read();

            // First handle endpoint 0 (the only control endpoint)
            if intstat_r.ep0out().bit_is_set() {
                ep_out |= bit;
            }
            if devcmdstat.read().setup().bit_is_set() {
                ep_setup |= bit;
            }

            if intstat_r.ep0in().bit_is_set() {
                intstat.write(|w| w.ep0in().set_bit());
                debug_assert!(intstat.read().ep0in().bit_is_clear());
                ep_in_complete |= bit;

                // EP0 needs manual toggling of Active bits
                // Weeelll interesting, not changing this makes no difference
                eps.eps[0].ep_in[0].modify(|_, w| w.a().not_active());
                // BELOW SEEMS NO LONGER NECESSARY!
                // prevents OUT-DATA-NAK
                // modify_endpoint!(endpoint_list, eps, EP0OUT, A: Active);
            }

            // non-CONTROL
            for ep in &self.endpoints[1..=self.max_endpoint] {
                bit <<= 1;
                let i = ep.index() as usize;

                // OUT = READ
                let out_offset = 2*i;
                let out_int = ((intstat_r.bits() >> out_offset) & 0x1) != 0;
                let out_inactive = eps.eps[i].ep_out[0].read().a().is_not_active();

                if out_int {
                    debug_assert!(out_inactive);
                    ep_out |= bit;
                    // EXPERIMENTAL: clear interrupt
                    // usb.intstat.write(|w| unsafe { w.bits(1u32 << out_offset) } );

                    // let err_code = usb.info.read().err_code().bits();
                    // let addr_set = devcmdstat.read().dev_addr().bits() > 0;
                    // if addr_set && err_code > 0 {
                    //     hprintln!("error {}", err_code).ok();
                    // }
                }

                // IN = WRITE
                let in_offset = 2*i + 1;
                let in_int = ((intstat_r.bits() >> in_offset) & 0x1) != 0;
                // WHYY is this sometimes still active?
                let in_inactive = eps.eps[i].ep_in[0].read().a().is_not_active();
                if in_int && !in_inactive {
                    // cortex_m_semihosting::hprintln!(
                    //     "IN is active for EP {}, but an IN interrupt fired", i,
                    // ).ok();
                    // cortex_m_semihosting::hprintln!(
                    //     "IntOnNAK_AI = {}, IntOnNAK_AO = {}",
                    //     devcmdstat.read().intonnak_ai().is_enabled(),
                    //     devcmdstat.read().intonnak_ao().is_enabled(),
                    // ).ok();

                    // debug_assert!(in_inactive);
                }
                if in_int && in_inactive {
                    ep_in_complete |= bit;
                    // clear it
                    usb.intstat.write(|w| unsafe { w.bits(1u32 << in_offset) } );
                    debug_assert!(eps.eps[i].ep_in[0].read().a().is_not_active());

                    // let err_code = usb.info.read().err_code().bits();
                    // let addr_set = devcmdstat.read().dev_addr().bits() > 0;
                    // if addr_set && err_code > 0 {
                    //     hprintln!("error {}", err_code).ok();
                    // }
                };
            }

            usb.intstat.write(|w| w.dev_int().set_bit());
            if (ep_out | ep_in_complete | ep_setup) != 0 {
                PollResult::Data { ep_out, ep_in_complete, ep_setup }
            } else {
                PollResult::None
            }
        })
    }

    fn read(&self, ep_addr: EndpointAddress, buf: &mut [u8]) -> Result<usize> {
        if !ep_addr.is_out() { return Err(UsbError::InvalidEndpoint); }

        interrupt::free(|cs| {
            let usb = self.usb_regs.borrow(cs);
            let eps = self.ep_regs.borrow(cs);
            self.endpoints[ep_addr.index()].read(buf, cs, usb, eps)
        })
    }

    fn write(&self, ep_addr: EndpointAddress, buf: &[u8]) -> Result<usize> {
        if !ep_addr.is_in() { return Err(UsbError::InvalidEndpoint); }

        interrupt::free(|cs| {
            let eps = self.ep_regs.borrow(cs);
            self.endpoints[ep_addr.index()].write(buf, cs, eps)
        })
    }

    fn set_stalled(&self, ep_addr: EndpointAddress, stalled: bool) {
        interrupt::free(|cs| {
            if self.is_stalled(ep_addr) == stalled { return }

            let i = ep_addr.index();
            let ep = &self.ep_regs.borrow(cs).eps[i];

            if i > 0 {
                match ep_addr.direction() {
                    UsbDirection::In => { while ep.ep_in[0].read().a().is_active() {} },
                    UsbDirection::Out => { while ep.ep_out[0].read().a().is_active() {} },
                }
            }

            match (stalled, ep_addr.direction()) {
                (true, UsbDirection::In) => ep.ep_in[0].modify(|_, w| w.s().stalled()),
                (true, UsbDirection::Out) => ep.ep_out[0].modify(|_, w| w.s().stalled()),

                (false, UsbDirection::In) => ep.ep_in[0].modify(|_, w| w.s().not_stalled()),
                (false, UsbDirection::Out) => ep.ep_out[0].modify(|_, w| w.s().not_stalled()),
            };
        });
    }

    fn is_stalled(&self, ep_addr: EndpointAddress) -> bool {
        interrupt::free(|cs| {
            let ep = &self.ep_regs.borrow(cs).eps[ep_addr.index()];
            match ep_addr.direction() {
                UsbDirection::In => ep.ep_in[0].read().s().is_stalled(),
                UsbDirection::Out => ep.ep_out[0].read().s().is_stalled(),
            }
        })
    }

    fn suspend(&self) {
        // cortex_m_semihosting::hprintln!("suspend not implemented!").unwrap();
        // interrupt::free(|cs| {
        //      self.regs.borrow(cs).cntr.modify(|_, w| w
        //         .fsusp().set_bit()
        //         .lpmode().set_bit());
        // });
    }

    fn resume(&self) {
        // cortex_m_semihosting::hprintln!("resume not implemented!").unwrap();
        // interrupt::free(|cs| {
        //     self.regs.borrow(cs).cntr.modify(|_, w| w
        //         .fsusp().clear_bit()
        //         .lpmode().clear_bit());
        // });
    }
}

