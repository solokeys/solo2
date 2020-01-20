//! main app in cortex-m-rtfm version
//!
//! See also `main_rt.rs` for a RT-only version.

#![no_std]
#![no_main]

use app::{board, hal};

#[allow(unused_imports)]
use cortex_m_semihosting::{dbg, hprintln};
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

    #[init(schedule = [toggle_red])]
    fn init(c: init::Context) -> init::LateResources {

        let (ctaphid, rgb, serial, usbd) = app::init_board(c.device, c.core);

        c.schedule.toggle_red(Instant::now() + PERIOD.cycles()).unwrap();

        init::LateResources {
            // authenticator,
            ctaphid,
            rgb,
            serial,
            usbd,
        }
    }

    // #[idle(resources = [ctaphid, serial, usbd])]
    #[idle(resources = [serial])]
    fn idle(c: idle::Context) -> ! {
        // let idle::Resources { mut ctaphid, mut serial, mut usbd } = c.resources;
        let idle::Resources { mut serial } = c.resources;

        loop {
            // not sure why we can't use `Exclusive` here, should we? how?
            // https://rtfm.rs/0.5/book/en/by-example/tips.html#generics
            // Important: do not pass unlocked serial :)
            app::drain_log_to_serial(&mut serial);

            // usbd.lock(|usbd| ctaphid.lock(|ctaphid| serial.lock(|serial|
            //     usbd.poll(&mut [ctaphid, serial])
            // )));
        }
    }

    #[task(binds = USB0_NEEDCLK, resources = [ctaphid, serial, usbd])]
    fn usb0_needclk(c: usb0_needclk::Context) {
        c.resources.usbd.poll(&mut [c.resources.ctaphid, c.resources.serial]);
    }

    #[task(binds = USB0, resources = [ctaphid, serial, usbd])]
    fn usb0(c: usb0::Context) {
        let before = Instant::now();
        c.resources.usbd.poll(&mut [c.resources.ctaphid, c.resources.serial]);
        let after = Instant::now();
        let length = (after - before).as_cycles();
        if length > 5_000 {
            info!("poll took {:?} cycles", length).ok();
        }
    }

    #[task(resources = [rgb], schedule = [toggle_red], priority = 3)]
    fn toggle_red(c: toggle_red::Context) {
        static mut TOGGLES: u32 = 1;
        use hal::traits::wg::digital::v2::ToggleableOutputPin;
        c.resources.rgb.red.toggle().ok();
        c.schedule.toggle_red(Instant::now() + PERIOD.cycles()).unwrap();
        info!("toggled red LED #{}", TOGGLES).ok();
        *TOGGLES += 1;
    }

    // something to dispatch software tasks from
    extern "C" {
        fn PLU();
    }

};
