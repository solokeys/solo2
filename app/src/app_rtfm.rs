//! main app in cortex-m-rtfm version
//!
//! See also `main_rt.rs` for a RT-only version.

#![no_std]
#![no_main]

use app::{board, hal};

// use cortex_m_semihosting::hprintln;
// use rtfm::Exclusive;
use funnel::info;

use rtfm::cyccnt::{Instant, U32Ext as _};
const PERIOD: u32 = 48_000_000;

#[rtfm::app(device = app::hal::raw, peripherals = true, monotonic = rtfm::cyccnt::CYCCNT)]
const APP: () = {

    struct Resources {
        ctaphid: app::types::CtapHidClass,
        rgb: board::led::Rgb,
        serial: app::types::SerialClass,
        usbd: app::types::Usbd,
    }

    #[idle(resources = [ctaphid, serial, usbd])]
    fn idle(mut c: idle::Context) -> ! {
        let idle::Resources { mut ctaphid, mut serial, mut usbd } = c.resources;

        loop {
            // not sure why we can't use `Exclusive` here, should we? how?
            // https://rtfm.rs/0.5/book/en/by-example/tips.html#generics
            // Important: do not pass unlocked serial :)
            app::drain_log_to_serial(&mut serial);

            usbd.lock(|usbd| ctaphid.lock(|ctaphid| serial.lock(|serial|
                usbd.poll(&mut [ctaphid, serial])
            )));
        }
    }

    #[init(schedule = [toggle_red])]
    fn init(c: init::Context) -> init::LateResources {

        let (ctaphid, mut rgb, serial, usbd) = app::init_board(c.device, c.core);

        c.schedule.toggle_red(Instant::now() + PERIOD.cycles()).unwrap();

        init::LateResources {
            // authenticator,
            ctaphid,
            rgb,
            serial,
            usbd,
        }
    }

    // #[task(binds = USB0_NEEDCLK, resources = [ctaphid, serial, usbd])]
    // fn usb0_needclk(c: usb0_needclk::Context) {
    //     c.resources.usbd.poll(&mut [c.resources.ctaphid, c.resources.serial]);
    // }

    #[task(binds = USB0, resources = [ctaphid, serial, usbd])]
    fn usb0(c: usb0::Context) {
        c.resources.usbd.poll(&mut [c.resources.ctaphid, c.resources.serial]);
        // these logs turn up but too noisy
        // info!("handled USB0 interrupt").ok();
    }

    #[task(resources = [rgb], schedule = [toggle_red], priority = 3)]
    fn toggle_red(c: toggle_red::Context) {
        use hal::traits::wg::digital::v2::ToggleableOutputPin;
        c.resources.rgb.red.toggle().ok();
        // these logs never turn up - why?
        info!("toggled red LED").unwrap();
        c.schedule.toggle_red(Instant::now() + PERIOD.cycles()).unwrap();
    }

    // something to dispatch software tasks from
    extern "C" {
        fn PLU();
    }

};
