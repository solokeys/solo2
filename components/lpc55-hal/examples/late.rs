//! examples/late.rs

#![deny(unsafe_code)]
// something something about:
//
// error: use of deprecated item
// 'cortex_m::peripheral::nvic::<impl cortex_m::peripheral::NVIC>::enable':
// Use `NVIC::unmask`
//
// #![deny(warnings)]
#![no_main]
#![no_std]

use cortex_m_semihosting::hprintln;
use hal::raw::Interrupt;
use heapless::{
    spsc::{Consumer, Producer, Queue},
};
use lpc55_hal as hal;
use panic_semihosting as _;

#[rtic::app(device = hal::raw)]
const APP: () = {
    // Late resources
    struct Resources {
        p: Producer<'static, u32, 4>,
        c: Consumer<'static, u32, 4>,
    }

    #[init]
    fn init(_: init::Context) -> init::LateResources {
        static mut Q: Queue<u32, 4> = Queue::new();

        let (p, c) = Q.split();

        // Initialization of late resources
        init::LateResources { p, c }
    }

    #[idle(resources = [c])]
    fn idle(c: idle::Context) -> ! {
        loop {
            if let Some(byte) = c.resources.c.dequeue() {
                hprintln!("received message: {}", byte).unwrap();
            // cortex_m::asm::wfi();
            } else {
                rtic::pend(Interrupt::ADC0);
            }
        }
    }

    #[task(binds = ADC0, resources = [p])]
    fn adc0(c: adc0::Context) {
        c.resources.p.enqueue(42).unwrap();
    }
};
