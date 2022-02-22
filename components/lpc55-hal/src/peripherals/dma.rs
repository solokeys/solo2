use crate::{
    raw,
    peripherals::{
        syscon::Syscon,
        adc::Adc,
        ctimer::Ctimer,
    },
    typestates::{
        init_state,
    }
};

#[repr(align(16))]
#[allow(dead_code)]
struct Descriptor{
    transfer_config: u32,
    source_end_addr: u32,
    dest_end_addr: u32,
    next: u32,
}

#[repr(align(512))]
struct Align512(
    Descriptor,Descriptor,Descriptor,Descriptor,
    Descriptor,Descriptor,Descriptor,Descriptor,
    Descriptor,Descriptor,Descriptor,Descriptor,
    Descriptor,Descriptor,Descriptor,Descriptor,

    Descriptor,Descriptor,Descriptor,Descriptor,
    Descriptor,Descriptor,Descriptor,Descriptor,
    Descriptor,Descriptor,Descriptor,Descriptor,
    Descriptor,Descriptor,Descriptor,Descriptor,
);

macro_rules! Empty {
    () => {
        Descriptor{transfer_config:0, source_end_addr:0, dest_end_addr:0, next:0}
    }
}

static mut DESCRIPTORS: Align512 = Align512(
    Empty!(),Empty!(),Empty!(),Empty!(),
    Empty!(),Empty!(),Empty!(),Empty!(),
    Empty!(),Empty!(),Empty!(),Empty!(),
    Empty!(),Empty!(),Empty!(),Empty!(),

    Empty!(),Empty!(),Empty!(),Empty!(),
    Empty!(),Empty!(),Empty!(),Empty!(),
    Empty!(),Empty!(),Empty!(),Empty!(),
    Empty!(),Empty!(),Empty!(),Empty!(),
);

crate::wrap_stateful_peripheral!(Dma, DMA0);

impl<State> Dma<State> {
    pub fn enabled(mut self, syscon: &mut Syscon) -> Dma<init_state::Enabled> {
        syscon.enable_clock(&mut self.raw);
        syscon.reset(&mut self.raw);

        self.raw.ctrl.write(|w| {w.enable().set_bit()});

        let descriptor_addr = unsafe {
            ((&DESCRIPTORS) as *const Align512) as u32
        };

        self.raw.srambase.write(|w|unsafe{w.bits(descriptor_addr)});

        Dma {
            raw: self.raw,
            _state: init_state::Enabled(()),
        }
    }

    pub fn disabled(mut self, syscon: &mut Syscon) -> Dma<init_state::Disabled> {
        syscon.disable_clock(&mut self.raw);
        Dma {
            raw: self.raw,
            _state: init_state::Disabled,
        }
    }

    /// Configures DMA to write any new results from ADC FIFO 0
    /// to a user supplied array in circular fashion.  Runs continuously.
    /// Timer is reset at the end of each ADC DMA transaction
    pub fn configure_adc(&mut self, adc: &mut Adc<init_state::Enabled>, timer: &mut impl Ctimer<init_state::Enabled>, recv_buf: &mut [u32]) {
        assert!(recv_buf.len() < 0x3FF);

        // channel 21 is ADC FIFO 0
        self.raw.channel21.cfg.write(|w| unsafe{
            w
            .periphreqen().set_bit()        // DMA blocks until ADC FIFO is ready
            .hwtrigen().clear_bit()         // Will software trigger
            .trigpol().clear_bit()          // falling edge
            .trigtype().clear_bit()         // edge sensitive
            .trigburst().clear_bit()        // No need to burst
            .chpriority().bits(1)           // 0 highest, 7 lowest
        });

        self.raw.channel21.xfercfg.write(|w| unsafe{
            w
            .cfgvalid().set_bit()           // channel descriptor will be valid (set below)
            .reload().set_bit()             // Reload next descriptor in .next pointer
            .swtrig().clear_bit()           // Dont start triggered
            .width().bit_32()               // u32 read from FIFO

            // *dst++ = FIFO
            .srcinc().no_increment()
            .dstinc().width_x_1()

            // total transferred will be (xfercount + 1)
            .xfercount().bits( (recv_buf.len() - 1) as u16)
        });

        self.raw.channel22.cfg.write(|w| unsafe{
            w
            .periphreqen().clear_bit()
            .hwtrigen().clear_bit()         // Will software trigger
            .trigpol().clear_bit()          // falling edge
            .trigtype().clear_bit()         // edge sensitive
            .trigburst().clear_bit()        // No need to burst
            .chpriority().bits(2)           // 0 highest, 7 lowest
        });

        // This is used simply to read from below.
        self.raw.channel22.xfercfg.write(|w| unsafe{
            w
            .cfgvalid().set_bit()           // channel descriptor will be valid (set below)
            .reload().set_bit()             // Reload next descriptor in .next pointer
            .swtrig().clear_bit()             // start triggered
            .width().bit_32()               // u32 read from FIFO

            // TC = 0
            .srcinc().no_increment()
            .dstinc().no_increment()

            // total transferred will be 0+1
            .xfercount().bits(1)
        });

        // Configure ping pong between ADC and sync timer (21 -> (22 <--> 23))
        // Note: 22, 23 are chosen simply because they aren't used.
        unsafe {
            DESCRIPTORS.21.transfer_config = 0; // first xferconfg is N/A

            // Get data from ADC FIFO A
            DESCRIPTORS.21.source_end_addr = (raw::ADC0::ptr() as u32) + 0x300;

            // End address should point to the last valid location DMA should write to.
            DESCRIPTORS.21.dest_end_addr = (recv_buf.as_mut_ptr() as u32) + (recv_buf.len() * 4 - 4) as u32;

            // Point to descriptor to reset sync timer
            DESCRIPTORS.21.next = ((&DESCRIPTORS.22) as *const Descriptor) as u32;


            DESCRIPTORS.22.transfer_config = self.raw.channel22.xfercfg.read().bits();

            // Get choose a memory location that contains zero
            DESCRIPTORS.22.source_end_addr = ((&DESCRIPTORS.0.source_end_addr) as *const u32) as u32;

            // Overwrite TC register
            DESCRIPTORS.22.dest_end_addr = ((timer.deref() as *const raw::ctimer0::RegisterBlock) as u32) + 0x08;

            // Point back to ADC descriptor to repeat
            DESCRIPTORS.22.next = ((&DESCRIPTORS.23) as *const Descriptor) as u32;


            // Use same config on reload
            DESCRIPTORS.23.transfer_config = self.raw.channel21.xfercfg.read().bits();

            // Get data from ADC FIFO A
            DESCRIPTORS.23.source_end_addr = (raw::ADC0::ptr() as u32) + 0x300;

            // End address should point to the last valid location DMA should write to.
            DESCRIPTORS.23.dest_end_addr = (recv_buf.as_mut_ptr() as u32) + (recv_buf.len() * 4 - 4) as u32;

            // Point to descriptor to reset sync timer
            DESCRIPTORS.23.next = ((&DESCRIPTORS.22) as *const Descriptor) as u32;
        }

        adc.de.write(|w| {
            w.fwmde0().set_bit() // Enable FIFO A dma
        });


        adc.fctrl[0].modify(|_,w| unsafe {
            w.fwmark().bits(2)  // when >2 samples in FIFO, dma request is issued.
        });

        // enable the starting channel
        self.raw.enableset0.write(|w| unsafe { w.bits( 1<<21 ) });

        // trigger
        self.raw.channel21.xfercfg.modify(|_,w| { w.swtrig().set_bit() });
    }
}
