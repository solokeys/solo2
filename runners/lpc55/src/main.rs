//! main app in cortex-m-rtic version
//!
//! See also `main_rt.rs` for a RT-only version.

#![no_std]
#![no_main]
// #![deny(warnings)]

use app::hal;
use hal::traits::wg::timer::Cancel;
use hal::traits::wg::timer::CountDown;
use hal::drivers::timer::Lap;
use hal::time::DurationExtensions;

use rtic::cyccnt::{Instant, U32Ext as _};

const CLOCK_FREQ: u32 = 96_000_000;
const PERIOD: u32 = CLOCK_FREQ/16;

const USB_INTERRUPT: board::hal::raw::Interrupt = board::hal::raw::Interrupt::USB1;
const NFC_INTERRUPT: board::hal::raw::Interrupt = board::hal::raw::Interrupt::PIN_INT0;

#[macro_use]
extern crate delog;
generate_macros!();

#[rtic::app(device = app::hal::raw, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {

    struct Resources {
        apdu_dispatch: app::types::ApduDispatch,
        ctaphid_dispatch: app::types::CtaphidDispach,
        trussed: app::types::Trussed,

        piv: app::types::Piv,
        totp: app::types::Totp,
        fido: app::types::FidoApp<fido_authenticator::NonSilentAuthenticator>,
        ndef: ndef_app::App<'static>,
        management: app::types::ManagementApp,

        usb_classes: Option<app::types::UsbClasses>,
        contactless: Option<app::types::Iso14443>,

        perf_timer: app::types::PerfTimer,

        clock_ctrl: Option<app::types::DynamicClockController>,
        hw_scheduler: app::types::HwScheduler,
    }

    #[init(schedule = [update_ui])]
    fn init(c: init::Context) -> init::LateResources {

        let (
            apdu_dispatch,
            ctaphid_dispatch,
            trussed,

            piv,
            totp,
            fido,
            ndef,
            management,

            usb_classes,
            contactless,

            perf_timer,
            clock_ctrl,
            hw_scheduler,
        ) = app::init_board(c.device, c.core);

        // don't toggle LED in passive mode
        if usb_classes.is_some() {
            hal::enable_cycle_counter();
            c.schedule.update_ui(Instant::now() + PERIOD.cycles()).unwrap();

        }

        init::LateResources {
            apdu_dispatch,
            ctaphid_dispatch,
            trussed,

            piv,
            totp,
            fido,
            ndef,
            management,

            usb_classes,
            contactless,

            perf_timer,

            clock_ctrl,
            hw_scheduler,
        }
    }

    #[idle(resources = [apdu_dispatch, ctaphid_dispatch, ndef, piv, totp, fido, management, perf_timer, usb_classes], schedule = [ccid_wait_extension])]
    fn idle(c: idle::Context) -> ! {
        let idle::Resources {
            apdu_dispatch,
            ctaphid_dispatch,
            ndef,
            piv,
            totp,
            fido,
            management,
            mut perf_timer,
            mut usb_classes,
        }
            = c.resources;

        let schedule = c.schedule;

        info_now!("inside IDLE");
        loop {

            let mut time = 0;
            perf_timer.lock(|perf_timer|{
                time = perf_timer.lap().0;
                if time == 60_000_000 {
                    perf_timer.start(60_000_000.microseconds());
                }
            });
            if time > 1_200_000 {
                app::Delogger::flush();
            }

            match apdu_dispatch.poll(&mut [ndef, piv, totp, fido, management]) {

                Some(apdu_dispatch::dispatch::Interface::Contact) => {
                    rtic::pend(USB_INTERRUPT);
                }
                Some(apdu_dispatch::dispatch::Interface::Contactless) => {
                    rtic::pend(NFC_INTERRUPT);
                }
                _ => {}
            }

            if ctaphid_dispatch.poll(&mut [fido, management]) {
                rtic::pend(USB_INTERRUPT);
            }

            usb_classes.lock(|usb_classes_maybe|{
                if usb_classes_maybe.is_some() {

                    let usb_classes = usb_classes_maybe.as_mut().unwrap();

                    usb_classes.ctaphid.check_timeout(time/1000);
                    usb_classes.poll();

                    match usb_classes.ccid.did_start_processing() {
                        usbd_ccid::types::Status::ReceivedData(duration) => {
                            schedule.ccid_wait_extension(
                                Instant::now() + (CLOCK_FREQ/1_000_000 * (duration.as_micros() as u32)).cycles()
                            ).ok();
                        }
                        _ => {}
                    }
                }
            });

        }
    }

    #[task(binds = USB1_NEEDCLK, resources = [], schedule = [], priority=6)]
    fn usb1_needclk(_c: usb1_needclk::Context) {
        // Behavior is same as in USB1 handler
        rtic::pend(USB_INTERRUPT);
    }

    /// Manages all traffic on the USB bus.
    #[task(binds = USB1, resources = [usb_classes], schedule = [ccid_wait_extension], priority=6)]
    fn usb(c: usb::Context) {
        let usb = unsafe { hal::raw::Peripherals::steal().USB1 } ;
        let before = Instant::now();
        let usb_classes = c.resources.usb_classes.as_mut().unwrap();

        //////////////
        usb_classes.poll();

        match usb_classes.ccid.did_start_processing() {
            usbd_ccid::types::Status::ReceivedData(duration) => {
                c.schedule.ccid_wait_extension(
                    Instant::now() + (CLOCK_FREQ/1_000_000 * (duration.as_micros() as u32)).cycles()
                ).ok();
            }
            _ => {}
        }
        //////////////

        let after = Instant::now();
        let length = (after - before).as_cycles();
        if length > 10_000 {
            debug!("poll took {:?} cycles", length);
        }
        let inten = usb.inten.read().bits();
        let intstat = usb.intstat.read().bits();
        let mask = inten & intstat;
        if mask != 0 {
            debug!("uncleared interrupts: {:?}", mask);
            for i in 0..5 {
                if mask & (1 << 2*i) != 0 {
                    debug!("EP{}OUT", i);
                }
                if mask & (1 << (2*i + 1)) != 0 {
                    debug!("EP{}IN", i);
                }
            }
            // Serial sends a stray 0x70 ("p") to CDC-ACM "data" OUT endpoint (3)
            // Need to fix that at the management, for now just clear that interrupt.
            usb.intstat.write(|w| unsafe{ w.bits(64) });
            // usb.intstat.write(|w| unsafe{ w.bits( usb.intstat.read().bits() ) });
        }


    }

    /// Whenever we start waiting for an application to reply to CCID, this must be scheduled.
    /// In case the application takes too long, this will periodically send wait extensions
    /// until the application replied.
    #[task(resources = [usb_classes], schedule = [ccid_wait_extension], priority = 6)]
    fn ccid_wait_extension(c: ccid_wait_extension::Context) {
        debug!("CCID WAIT EXTENSION");
        let status = c.resources.usb_classes.as_mut().unwrap().ccid.send_wait_extension();
        match status {
            usbd_ccid::types::Status::ReceivedData(duration) => {
                c.schedule.ccid_wait_extension(
                    Instant::now() + (CLOCK_FREQ/1_000 * (duration.as_millis() as u32)).cycles()
                ).ok();
            }
            _ => {}
        }
    }

    #[task(binds = MAILBOX, resources = [usb_classes], priority = 5)]
    #[allow(unused_mut,unused_variables)]
    fn mailbox(mut c: mailbox::Context) {
        #[cfg(feature = "log-serial")]
        c.resources.usb_classes.lock(|usb_classes_maybe| {
            match usb_classes_maybe.as_mut() {
                Some(usb_classes) => {
                    // usb_classes.serial.write(logs.as_bytes()).ok();
                    usb_classes.serial.write(b"dummy test string\n").ok();
                    // app::drain_log_to_serial(&mut usb_classes.serial);
                }
                _=>{}
            }
        });
        // // let usb_classes = c.resources.usb_classes.as_mut().unwrap();

        // let mailbox::Resources { usb_classes } = c.resources;
        // let x: () = usb_classes;
        // // if let Some(usb_classes) = usb_classes.as_mut() {
        // //     usb_classes.serial.write(b"dummy test string\n").ok();
        // // }
    }

    #[task(binds = OS_EVENT, resources = [trussed], priority = 5)]
    fn os_event(c: os_event::Context) {
        c.resources.trussed.process();
    }

    #[task(resources = [trussed], schedule = [update_ui], priority = 1)]
    fn update_ui(mut c: update_ui::Context) {

        static mut UPDATES: u32 = 1;

        // let wait_periods = c.resources.trussed.lock(|trussed| trussed.update_ui());
        c.resources.trussed.lock(|trussed| trussed.update_ui());
        // c.schedule.update_ui(Instant::now() + wait_periods * PERIOD.cycles()).unwrap();
        c.schedule.update_ui(Instant::now() + PERIOD.cycles()).unwrap();

        *UPDATES += 1;
    }



    #[task(binds = CTIMER0, resources = [contactless, perf_timer, hw_scheduler], priority = 7)]
    fn nfc_wait_extension(c: nfc_wait_extension::Context) {
        let nfc_wait_extension::Resources {
            contactless,
            perf_timer: _perf_timer,
            hw_scheduler,
        }
            = c.resources;
        if let Some(contactless) = contactless.as_mut() {

            // clear the interrupt
            hw_scheduler.cancel().ok();

            info!("<{}", _perf_timer.lap().0/100);
            let status = contactless.poll_wait_extensions();
            match status {
                nfc_device::Iso14443Status::Idle => {}
                nfc_device::Iso14443Status::ReceivedData(duration) => {
                    hw_scheduler.start(duration.subsec_micros().microseconds());
                }
            }
            info!(" {}>", _perf_timer.lap().0/100);
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
        let _starttime = perf_timer.lap().0/100;

        info!("[");
        let status = contactless.poll();
        match status {
            nfc_device::Iso14443Status::Idle => {}
            nfc_device::Iso14443Status::ReceivedData(duration) => {
                hw_scheduler.cancel().ok();
                hw_scheduler.start(duration.subsec_micros().microseconds());
            }
        }
        info!("{}-{}]", _starttime, perf_timer.lap().0/100);

        perf_timer.cancel().ok();
        perf_timer.start(60_000_000.microseconds());
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
        fn PIN_INT5();
        fn PIN_INT7();
    }

};
