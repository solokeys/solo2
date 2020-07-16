//! main app in cortex-m-rtic version
//!
//! See also `main_rt.rs` for a RT-only version.

#![no_std]
#![no_main]

use app::{board, hal};
use lpc55_hal::drivers::timer::Lap;
use hal::traits::wg::timer::Cancel;
use hal::traits::wg::timer::CountDown;
use hal::time::*;

use funnel::info;

#[cfg(feature = "debug-app")]
#[macro_use(debug)]
extern crate funnel;

#[cfg(not(feature = "debug-app"))]
#[macro_use]
macro_rules! debug { ($($tt:tt)*) => {{ core::result::Result::<(), core::convert::Infallible>::Ok(()) }} }

use rtic::cyccnt::{Instant, U32Ext as _};

const CLOCK_FREQ: u32 = 96_000_000;
const PERIOD: u32 = CLOCK_FREQ/2;

#[rtic::app(device = app::hal::raw, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {

    struct Resources {
        authnr: app::types::Authenticator,
        apdu_dispatch: app::types::ApduDispatch,
        trussed: app::types::CryptoService,

        piv: app::types::Piv,
        fido: Option<app::types::FidoApplet>,
        ndef: applet_ndef::NdefApplet<'static>,

        usb_classes: Option<app::types::UsbClasses>,
        contactless: Option<app::types::Iso14443>,

        perf_timer: app::types::PerfTimer,
        rgb: board::led::RgbLed,
        three_buttons: Option<board::button::ThreeButtons>,

        clock_ctrl: Option<app::types::DynamicClockController>,
        hw_scheduler: app::types::HwScheduler,
    }

    #[init(schedule = [toggle_red])]
    fn init(c: init::Context) -> init::LateResources {

        let (
            authnr,
            apdu_dispatch,
            trussed,

            piv,
            fido,
            ndef,

            usb_classes,
            contactless,

            perf_timer,
            rgb,
            three_buttons,
            clock_ctrl,
            hw_scheduler,
        ) = app::init_board(c.device, c.core);

        // don't toggle LED in passive mode
        if usb_classes.is_some() {
            c.schedule.toggle_red(Instant::now() + PERIOD.cycles()).unwrap();
        }

        init::LateResources {
            authnr,
            apdu_dispatch,
            trussed,

            piv,
            fido,
            ndef,

            usb_classes,
            contactless,

            perf_timer,
            rgb,
            three_buttons,

            clock_ctrl,
            hw_scheduler,
        }
    }

    #[idle(resources = [authnr, usb_classes, apdu_dispatch, ndef, piv, fido, contactless, perf_timer])]
    fn idle(c: idle::Context) -> ! {
        let idle::Resources {
            authnr,
            mut usb_classes,
            apdu_dispatch,
            ndef,
            piv,
            fido,
            mut contactless,
            mut perf_timer,
        }
            = c.resources;

        loop {

            let mut time = 0;
            perf_timer.lock(|perf_timer|{
                time = perf_timer.lap().0;
            });
            if time > 1_000_000 {
                // Only drain outside of a 1s window of any NFC activity.
                #[cfg(feature = "log-serial")]
                app::drain_log_to_serial(&mut serial);
                #[cfg(feature = "log-semihosting")]
                app::drain_log_to_semihosting();
            }

            match fido.as_mut() {
                Some(fido) => {
                    apdu_dispatch.poll(&mut [ndef, piv, fido]);
                }
                _ => {
                    apdu_dispatch.poll(&mut [ndef, piv]);
                }
            }

            contactless.lock(|contactless|  {
                match contactless.as_ref() {
                    Some(contactless) => {
                        if contactless.is_ready_to_transmit() {
                            rtic::pend(lpc55_hal::raw::Interrupt::PIN_INT0);
                        }
                    }
                    _ => {}
                }
            });

            usb_classes.lock(|usb_classes_maybe| {
                match usb_classes_maybe.as_mut() {
                    Some(usb_classes) => {
                        usb_classes.ctaphid.check_for_responses();
                        // the `usbd.poll` only calls its classes if
                        // there is activity on the bus. hence we need
                        // to kick ccid to pick up responses...
                        usb_classes.ccid.sneaky_poll();
                        usb_classes.usbd.poll(&mut [
                            &mut usb_classes.ccid,
                            &mut usb_classes.ctaphid,
                            &mut usb_classes.serial
                        ]);
                    }
                    _ => {}
                }
            } );


            authnr.poll();
            // piv.poll();
        }
    }

    #[task(binds = USB1_NEEDCLK, resources = [usb_classes], priority=5)]
    fn usb1_needclk(c: usb1_needclk::Context) {
        let usb_classes = c.resources.usb_classes.as_mut().unwrap();
        usb_classes.usbd.poll(&mut [&mut usb_classes.ccid, &mut usb_classes.ctaphid, &mut usb_classes.serial]);
    }

    #[task(binds = USB1, resources = [usb_classes], priority=5)]
    fn usb1(c: usb1::Context) {
        let usb = unsafe { hal::raw::Peripherals::steal().USB1 } ;
        let before = Instant::now();
        let usb_classes = c.resources.usb_classes.as_mut().unwrap();

        //////////////
        usb_classes.usbd.poll(&mut [&mut usb_classes.ccid, &mut usb_classes.ctaphid, &mut usb_classes.serial]);
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

    #[task(binds = OS_EVENT, resources = [trussed], priority = 7)]
    fn os_event(c: os_event::Context) {
        c.resources.trussed.process();
    }

    #[task(resources = [rgb], schedule = [toggle_red], priority = 1)]
    fn toggle_red(c: toggle_red::Context) {

        static mut TOGGLES: u32 = 1;
        static mut ON: bool = false;
        use solo_bee_traits::rgb_led::RgbLed;
        if *ON {
            c.resources.rgb.turn_off();
            *ON = false;
        } else {
            c.resources.rgb.green(10);
            *ON = true;
        }

        c.schedule.toggle_red(Instant::now() + PERIOD.cycles()).unwrap();
        info!("toggled red LED #{}", TOGGLES).ok();

        *TOGGLES += 1;
    }

    // #[task( binds = CTIMER0, resources = [hw_scheduler], priority = 7)]
    // fn nfc_wait_extension(c: nfc_wait_extension::Context) {
    //     info!("HW BLINK").ok();
    //     let hw_blink::Resources {
    //         hw_scheduler,
    //     } = c.resources;
    //     hw_scheduler.start(500.ms());
    // }

    #[task(binds = CTIMER0, resources = [contactless, perf_timer, hw_scheduler], priority = 7)]
    fn nfc_wait_extension(c: nfc_wait_extension::Context) {
        let nfc_wait_extension::Resources {
            contactless,
            perf_timer,
            hw_scheduler,
        }
            = c.resources;
        if let Some(contactless) = contactless.as_mut() {

            // clear the interrupt
            hw_scheduler.cancel().ok();

            info!("<{}", perf_timer.lap().0/100).ok();
            let status = contactless.poll_wait_extensions();
            match status {
                iso14443::Iso14443Status::Idle => {}
                iso14443::Iso14443Status::ReceivedData(duration) => {
                    hw_scheduler.start(duration.subsec_millis().ms());
                }
            }
            info!(" {}>", perf_timer.lap().0/100).ok();
        }
    }

    #[task(binds = PIN_INT0, resources = [
            contactless, perf_timer, hw_scheduler,
        ], priority = 7,
    )]
    fn nfc_irq(c: nfc_irq::Context) {

        let nfc_irq::Resources {
            contactless,
            perf_timer,
            hw_scheduler,
            }
            = c.resources;
        let contactless = contactless.as_mut().unwrap();

        info!("[{}", perf_timer.lap().0/100).ok();
        let status = contactless.poll();
        match status {
            iso14443::Iso14443Status::Idle => {}
            iso14443::Iso14443Status::ReceivedData(duration) => {
                hw_scheduler.start(duration.subsec_millis().ms());
            }
        }
        info!(" {}]", perf_timer.lap().0/100).ok();

        perf_timer.cancel().ok();
        perf_timer.start(60_000.ms());
    }

    #[task(binds = ADC0, resources = [clock_ctrl], priority = 8)]
    fn adc_int(c: adc_int::Context) {
        let adc_int::Resources {
            clock_ctrl,
        } = c.resources;
        clock_ctrl.as_mut().unwrap().handle();
    }


    // something to dispatch software tasks from
    extern "C" {
        fn PLU();
        fn PIN_INT7();
    }

};
