//! main app in cortex-m-rtfm version
//!
//! See also `main_rt.rs` for a RT-only version.

#![no_std]
#![no_main]

use app::{board, hal};
// use fido_authenticator::{
//     Authenticator,
// };

// use rtfm::Exclusive;
use funnel::info;

#[cfg(feature = "debug-app")]
#[macro_use(debug)]
extern crate funnel;

#[cfg(not(feature = "debug-app"))]
#[macro_use]
macro_rules! debug { ($($tt:tt)*) => {{ core::result::Result::<(), core::convert::Infallible>::Ok(()) }} }

use rtfm::cyccnt::{Instant, U32Ext as _};
const PERIOD: u32 = 1*48_000_000;

#[rtfm::app(device = app::hal::raw, peripherals = true, monotonic = rtfm::cyccnt::CYCCNT)]
const APP: () = {

    struct Resources {
        authnr: app::types::Authenticator,
        ccid: app::types::CcidClass,
        crypto: app::types::CryptoService,
        ctaphid: app::types::CtapHidClass,
        piv: app::types::Piv,
        rgb: board::led::RgbLed,
        serial: app::types::SerialClass,
        usbd: app::types::Usbd,
        // os_channels: fido_authenticator::OsChannels,
    }

    #[init(schedule = [toggle_red])]
    fn init(c: init::Context) -> init::LateResources {

        let (authnr, ccid, crypto, ctaphid, piv, rgb, serial, usbd) = app::init_board(c.device, c.core);

        c.schedule.toggle_red(Instant::now() + PERIOD.cycles()).unwrap();

        init::LateResources {
            authnr,
            ccid,
            crypto,
            ctaphid,
            piv,
            rgb,
            serial,
            usbd,
        }
    }

    #[idle(resources = [authnr, ccid, ctaphid, piv, serial, usbd])]
    fn idle(c: idle::Context) -> ! {
        let idle::Resources { authnr, mut ccid, mut ctaphid, piv, mut serial, mut usbd }
            = c.resources;

        loop {
            // not sure why we can't use `Exclusive` here, should we? how?
            // https://rtfm.rs/0.5/book/en/by-example/tips.html#generics
            // Important: do not pass unlocked serial :)
            // cortex_m_semihosting::hprintln!("idle loop").ok();
            #[cfg(feature = "log-serial")]
            app::drain_log_to_serial(&mut serial);
            #[cfg(feature = "log-semihosting")]
            app::drain_log_to_semihosting();

            usbd.lock(|usbd| ccid.lock(|ccid| ctaphid.lock(|ctaphid| serial.lock(|serial| {
                ctaphid.check_for_responses();
                // the `usbd.poll` only calls its classes if
                // there is activity on the bus. hence we need
                // to kick ccid to pick up responses...
                ccid.sneaky_poll();
                usbd.poll(&mut [ccid, ctaphid, serial])
            } ))));

            authnr.poll();
            piv.poll();


            // // NEW
            // if let Some(request) = ctaphid.lock(|ctaphid| ctaphid.request()) {
            //     // the potentially time-consuming part
            //     let response = authenticator.pre_process(request);
            //     ctaphid.lock(|ctaphid| ctaphid.response(response));
            //     rtfm::pend(Interrupt::USB0);
            // }
        }
    }

    #[task(binds = USB0_NEEDCLK, resources = [ccid, ctaphid, serial, usbd], priority=5)]
    fn usb0_needclk(c: usb0_needclk::Context) {
        c.resources.usbd.poll(&mut [c.resources.ccid, c.resources.ctaphid, c.resources.serial]);
    }

    #[task(binds = USB0, resources = [ccid, ctaphid, serial, usbd], priority=5)]
    fn usb0(c: usb0::Context) {
        let usb = unsafe { hal::raw::Peripherals::steal().USB0 } ;
        // cortex_m_semihosting::hprintln!("handler intstat = {:x}", usb0.intstat.read().bits()).ok();
        // cortex_m_semihosting::hprintln!("handler inten = {:x}", usb0.inten.read().bits()).ok();
        // c.resources.usbd.clear_interrupt();
        let before = Instant::now();

        //////////////
        c.resources.usbd.poll(&mut [c.resources.ccid, c.resources.ctaphid, c.resources.serial]);
        //////////////

        let after = Instant::now();
        let length = (after - before).as_cycles();
        if length > 10_000 {
            info!("poll took {:?} cycles", length).ok();
        }
        let inten = usb.inten.read().bits();
        let intstat = usb.intstat.read().bits();
        let mask = inten & intstat;
        if mask != 0 {
            debug!("uncleared interrupts: {:?}", mask).ok();
            for i in 0..5 {
                if mask & (1 << 2*i) != 0 {
                    debug!("EP{}OUT", i).ok();
                }
                if mask & (1 << (2*i + 1)) != 0 {
                    debug!("EP{}IN", i).ok();
                }
            }
            // Serial sends a stray 0x70 ("p") to CDC-ACM "data" OUT endpoint (3)
            // Need to fix that at the root, for now just clear that interrupt.
            usb.intstat.write(|w| unsafe{ w.bits(64) });
            // usb.intstat.write(|w| unsafe{ w.bits( usb.intstat.read().bits() ) });
        }

    }

    #[task(binds = OS_EVENT, resources = [crypto], priority = 7)]
    fn os_event(c: os_event::Context) {
        c.resources.crypto.process();
    }

    // #[task(binds = OS_EVENT, resources = [os_channels], priority = 7)]
    // fn os_event(c: os_event::Context) {
    //     let os_event::Resources { mut os_channels, .. } = c.resources;
    //     if let Some(msg) = os_channels.recv.dequeue() {
    //         match msg {
    //             AuthnrToOsMessages::Heya(string) => { hprintln!("got a syscall: {}", &string).ok(); }
    //             _ => { hprintln!("got a syscall!").ok(); }
    //         }
    //     }
    // }

    #[task(resources = [ctaphid, rgb], schedule = [toggle_red], priority = 1)]
    fn toggle_red(c: toggle_red::Context) {

        static mut TOGGLES: u32 = 1;
        static mut ON: bool = false;
        use solo_bee_traits::rgb_led::RgbLed;
        if *ON {
            c.resources.rgb.red(0);
            *ON = false;
        } else {
            c.resources.rgb.red(200);
            *ON = true;
        }
        c.schedule.toggle_red(Instant::now() + PERIOD.cycles()).unwrap();
        // debug!("{}:{} toggled red LED #{}", file!(), line!(), TOGGLES).ok();
        debug!("toggled red LED #{}", TOGGLES).ok();

        // let sig_count = c.resources.ctaphid.borrow_mut_authenticator()
        //     .signature_counter().expect("issue reading sig count");
        // hprintln!("sigs: {}", sig_count).ok();
        *TOGGLES += 1;
    }

    // something to dispatch software tasks from
    extern "C" {
        fn PLU();
    }

};
