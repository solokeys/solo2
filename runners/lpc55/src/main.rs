//! main app in cortex-m-rtic version
//!
//! See also `main_rt.rs` for a RT-only version.

#![no_std]
#![no_main]
// #![deny(warnings)]

// FIXME: Disable these once rtic is updated, as these originate there.
#![allow(unexpected_cfgs)]
#![allow(static_mut_refs)]
#![allow(non_local_definitions)]

const REFRESH_MILLISECS: u64 = 50;

const USB_INTERRUPT: board::hal::raw::Interrupt = board::hal::raw::Interrupt::USB1;
const NFC_INTERRUPT: board::hal::raw::Interrupt = board::hal::raw::Interrupt::PIN_INT0;

#[macro_use]
extern crate delog;
generate_macros!();

use core::arch::asm;

#[inline]
pub fn msp() -> u32 {
    let r;
    unsafe { asm!("mrs {}, MSP", out(reg) r, options(nomem, nostack, preserves_flags)) };
    r
}

#[rtic::app(device = runner::hal::raw, peripherals = true, dispatchers = [PLU, PIN_INT5, PIN_INT7])]
mod app {
    use hal::drivers::timer::Elapsed;
    use hal::time::{DurationExtensions, Microseconds};
    use hal::traits::wg::timer::Cancel;
    use hal::traits::wg::timer::CountDown;
    use runner::hal;

    use systick_monotonic::fugit::TimerDurationU64;
    use systick_monotonic::Systick;

    use crate::{NFC_INTERRUPT, REFRESH_MILLISECS, USB_INTERRUPT};

    #[monotonic(binds = SysTick, default = true)]
    type MyMono = Systick<1000>;

    #[local]
    struct LocalResources {
        updates: u32,
    }

    #[shared]
    struct SharedResources {
        /// Dispatches APDUs from contact+contactless interface to apps.
        #[lock_free]
        apdu_dispatch: runner::types::ApduDispatch,

        /// Dispatches CTAPHID messages to apps.
        #[lock_free]
        ctaphid_dispatch: runner::types::CtaphidDispatch,

        /// The Trussed service, used by all applications.
        trussed: runner::types::Trussed,

        /// All the applications that the device serves.
        #[lock_free]
        apps: runner::types::Apps,

        /// The USB driver classes
        usb_classes: Option<runner::types::UsbClasses>,
        /// The NFC driver
        #[lock_free]
        contactless: Option<runner::types::Iso14443>,

        /// This timer is used while developing NFC, to time how long things took,
        /// and to make sure logs are not flushed in the middle of NFC transactions.
        ///
        /// It could and should be behind some kind of `debug-nfc-timer` feature flag.
        perf_timer: runner::types::PerformanceTimer,

        /// When using passive power (i.e. NFC), we switch between 12MHz
        /// and 48Mhz, trying to optimize speed while keeping power high enough.
        ///
        /// In principle, we could just run at 12MHz constantly, and then
        /// there would be no need for a system-speed independent wait extender.
        #[lock_free]
        clock_ctrl: Option<runner::types::DynamicClockController>,

        /// Applications must respond to NFC requests within a certain time frame (~40ms)
        /// or send a "wait extension" to the NFC reader. This timer is responsible
        /// for scheduling these.
        ///
        /// In the current version of RTIC, the built-in scheduling cannot be used, as it
        /// is expressed in terms of cycles, and our dynamic clock control potentially changes
        /// timing. It seems like RTIC v6 will allow using such a timer directly.
        ///
        /// Alternatively, we could send wait extensions as if always running at 12MHz,
        /// which would cause more context switching and NFC exchangs though.
        ///
        /// NB: CCID + CTAPHID also have a sort of "wait extension" implemented, however
        /// since the system runs at constant speed when powered over USB, there is no
        /// need for such an independent timer.
        #[lock_free]
        wait_extender: runner::types::NfcWaitExtender,
    }

    #[init]
    fn init(c: init::Context) -> (SharedResources, LocalResources, init::Monotonics) {
        let (
            apdu_dispatch,
            ctaphid_dispatch,
            trussed,
            apps,
            usb_classes,
            contactless,
            perf_timer,
            clock_ctrl,
            wait_extender,
            monotonic,
        ) = runner::init_board(c.device, c.core);

        // don't toggle LED in passive mode
        if usb_classes.is_some() {
            hal::enable_cycle_counter();
            // c.schedule.update_ui(Instant::now() + PERIOD.cycles()).unwrap();
            update_ui::spawn_after(TimerDurationU64::millis(REFRESH_MILLISECS)).unwrap();
        }

        (
            SharedResources {
                apdu_dispatch,
                ctaphid_dispatch,
                trussed,

                apps,

                usb_classes,
                contactless,

                perf_timer,

                clock_ctrl,
                wait_extender,
            },
            LocalResources { updates: 1, },
            init::Monotonics(monotonic),
        )
    }

    #[idle(shared = [apdu_dispatch, ctaphid_dispatch, apps, perf_timer, usb_classes])]
    fn idle(mut c: idle::Context) -> ! {
        info_now!("inside IDLE, initial SP = {:08X}", msp());
        loop {
            let mut time = 0;
            c.shared.perf_timer.lock(|perf_timer| {
                time = perf_timer.elapsed().0;
                if time == 60_000_000 {
                    perf_timer.start(60_000_000.microseconds());
                }
            });
            if time > 1_200_000 {
                runner::Delogger::flush();
            }

            match c
                .shared
                .apps
                .apdu_dispatch(|apps| c.shared.apdu_dispatch.poll(apps))
            {
                Some(apdu_dispatch::dispatch::Interface::Contact) => {
                    rtic::pend(USB_INTERRUPT);
                }
                Some(apdu_dispatch::dispatch::Interface::Contactless) => {
                    rtic::pend(NFC_INTERRUPT);
                }
                _ => {}
            }

            if c.shared
                .apps
                .ctaphid_dispatch(|apps| c.shared.ctaphid_dispatch.poll(apps))
            {
                rtic::pend(USB_INTERRUPT);
            }

            c.shared.usb_classes.lock(|usb_classes_maybe| {
                if usb_classes_maybe.is_some() {
                    let usb_classes = usb_classes_maybe.as_mut().unwrap();

                    usb_classes.ctaphid.check_timeout(time / 1000);
                    usb_classes.poll();

                    match usb_classes.ccid.did_start_processing() {
                        usbd_ccid::types::Status::ReceivedData(milliseconds) => {
                            ccid_wait_extension::spawn_after(TimerDurationU64::millis(
                                milliseconds.0.into(),
                            ))
                            .ok();
                        }
                        _ => {}
                    }

                    match usb_classes.ctaphid.did_start_processing() {
                        usbd_ctaphid::types::Status::ReceivedData(milliseconds) => {
                            ctaphid_keepalive::spawn_after(TimerDurationU64::millis(
                                milliseconds.0.into(),
                            ))
                            .ok();
                        }
                        _ => {}
                    }
                }
            });
        }
    }

    #[task(binds = USB1_NEEDCLK, priority=6)]
    fn usb1_needclk(_c: usb1_needclk::Context) {
        // Behavior is same as in USB1 handler
        rtic::pend(USB_INTERRUPT);
    }

    /// Manages all traffic on the USB bus.
    #[task(binds = USB1, shared = [usb_classes], priority=6)]
    fn usb(mut c: usb::Context) {
        // let remaining = msp() - 0x2000_0000;
        // if remaining < 100_000 {
        //     debug_now!("USB interrupt: remaining stack size: {} bytes", remaining);
        // }
        let usb = unsafe { hal::raw::Peripherals::steal().USB1 };
        // let before = Instant::now();
        c.shared.usb_classes.lock(|usb_classes_maybe| {
            let usb_classes = usb_classes_maybe.as_mut().unwrap();

            //////////////
            // if remaining < 60_000 {
            //     debug_now!("polling usb classes");
            // }
            usb_classes.poll();

            match usb_classes.ccid.did_start_processing() {
                usbd_ccid::types::Status::ReceivedData(milliseconds) => {
                    // if remaining < 60_000 {
                    //     debug_now!("scheduling CCID wait extension");
                    // }
                    ccid_wait_extension::spawn_after(TimerDurationU64::millis(
                        milliseconds.0.into(),
                    ))
                    .ok();
                }
                _ => {}
            }
            match usb_classes.ctaphid.did_start_processing() {
                usbd_ctaphid::types::Status::ReceivedData(milliseconds) => {
                    // if remaining < 60_000 {
                    //     debug_now!("scheduling CTAPHID wait extension");
                    // }
                    ctaphid_keepalive::spawn_after(TimerDurationU64::millis(milliseconds.0.into()))
                        .ok();
                }
                _ => {}
            }
        });
        //////////////

        // let after = Instant::now();
        // let length = (after - before).as_cycles();
        // if length > 10_000 {
        //     // debug!("poll took {:?} cycles", length);
        // }
        let inten = usb.inten.read().bits();
        let intstat = usb.intstat.read().bits();
        let mask = inten & intstat;
        if mask != 0 {
            for i in 0..5 {
                if mask & (1 << 2 * i) != 0 {
                    // debug!("EP{}OUT", i);
                }
                if mask & (1 << (2 * i + 1)) != 0 {
                    // debug!("EP{}IN", i);
                }
            }
            // Serial sends a stray 0x70 ("p") to CDC-ACM "data" OUT endpoint (3)
            // Need to fix that at the management, for now just clear that interrupt.
            usb.intstat.write(|w| unsafe { w.bits(64) });
            // usb.intstat.write(|w| unsafe{ w.bits( usb.intstat.read().bits() ) });
        }

        // if remaining < 60_000 {
        //     debug_now!("USB interrupt done: {} bytes", remaining);
        // }
    }

    /// Whenever we start waiting for an application to reply to CCID, this must be scheduled.
    /// In case the application takes too long, this will periodically send wait extensions
    /// until the application replied.
    #[task(shared = [usb_classes], priority = 6)]
    fn ccid_wait_extension(mut c: ccid_wait_extension::Context) {
        debug_now!("CCID WAIT EXTENSION");
        debug_now!("remaining stack size: {} bytes", msp() - 0x2000_0000);
        let status = c.shared.usb_classes.lock(|usb_classes_maybe| {
            usb_classes_maybe
                .as_mut()
                .unwrap()
                .ccid
                .send_wait_extension()
        });
        match status {
            usbd_ccid::types::Status::ReceivedData(milliseconds) => {
                ccid_wait_extension::spawn_after(TimerDurationU64::millis(milliseconds.0.into()))
                    .ok();
            }
            _ => {}
        }
    }

    /// Same as with CCID, but sending ctaphid keepalive statuses.
    #[task(shared = [usb_classes], priority = 6)]
    fn ctaphid_keepalive(mut c: ctaphid_keepalive::Context) {
        debug_now!("CTAPHID keepalive");
        debug_now!("remaining stack size: {} bytes", msp() - 0x2000_0000);
        let status = c.shared.usb_classes.lock(|usb_classes_maybe| {
            usb_classes_maybe
                .as_mut()
                .unwrap()
                .ctaphid
                .send_keepalive(board::trussed::UserPresenceStatus::waiting())
        });
        match status {
            usbd_ctaphid::types::Status::ReceivedData(milliseconds) => {
                ctaphid_keepalive::spawn_after(TimerDurationU64::millis(milliseconds.0.into()))
                    .ok();
            }
            _ => {}
        }
    }

    #[task(binds = MAILBOX, shared = [usb_classes], priority = 5)]
    #[allow(unused_mut, unused_variables)]
    fn mailbox(mut c: mailbox::Context) {
        // debug_now!("mailbox: remaining stack size: {} bytes", msp() - 0x2000_0000);
        #[cfg(feature = "log-serial")]
        c.resources.usb_classes.lock(|usb_classes_maybe| {
            match usb_classes_maybe.as_mut() {
                Some(usb_classes) => {
                    // usb_classes.serial.write(logs.as_bytes()).ok();
                    usb_classes.serial.write(b"dummy test string\n").ok();
                    // app::drain_log_to_serial(&mut usb_classes.serial);
                }
                _ => {}
            }
        });
        // // let usb_classes = c.resources.usb_classes.as_mut().unwrap();

        // let mailbox::Resources { usb_classes } = c.resources;
        // let x: () = usb_classes;
        // // if let Some(usb_classes) = usb_classes.as_mut() {
        // //     usb_classes.serial.write(b"dummy test string\n").ok();
        // // }
    }

    #[task(binds = OS_EVENT, shared = [trussed], priority = 5)]
    fn os_event(mut c: os_event::Context) {
        // debug_now!("os event: remaining stack size: {} bytes", msp() - 0x2000_0000);
        c.shared.trussed.lock(|trussed| trussed.process());
    }

    #[task(shared = [trussed], local = [updates], priority = 1)]
    fn update_ui(mut c: update_ui::Context) {
        // debug_now!("update UI: remaining stack size: {} bytes", msp() - 0x2000_0000);

        // let wait_periods = c.resources.trussed.lock(|trussed| trussed.update_ui());
        c.shared.trussed.lock(|trussed| trussed.update_ui());
        // c.schedule.update_ui(Instant::now() + wait_periods * PERIOD.cycles()).unwrap();
        update_ui::spawn_after(TimerDurationU64::millis(REFRESH_MILLISECS)).ok();

        *c.local.updates += 1;
    }

    #[task(binds = CTIMER0, shared = [contactless, perf_timer, wait_extender], priority = 7)]
    fn nfc_wait_extension(c: nfc_wait_extension::Context) {
        if let Some(contactless) = c.shared.contactless.as_mut() {
            // clear the interrupt
            c.shared.wait_extender.cancel().ok();

            info!("<{}", _perf_timer.elapsed().0 / 100);
            let status = contactless.poll_wait_extensions();
            match status {
                nfc_device::Iso14443Status::Idle => {}
                nfc_device::Iso14443Status::ReceivedData(milliseconds) => {
                    c.shared
                        .wait_extender
                        .start(Microseconds::try_from(milliseconds).unwrap());
                }
            }
            info!(" {}>", _perf_timer.elapsed().0 / 100);
        }
    }

    #[task(binds = PIN_INT0, shared = [
            contactless, perf_timer, wait_extender,
        ], priority = 7,
    )]
    fn nfc_irq(mut c: nfc_irq::Context) {
        c.shared.perf_timer.lock(|perf_timer| {
            let contactless = c.shared.contactless.as_mut().unwrap();
            let _starttime = perf_timer.elapsed().0 / 100;

            info!("[");
            let status = contactless.poll();
            match status {
                nfc_device::Iso14443Status::Idle => {}
                nfc_device::Iso14443Status::ReceivedData(milliseconds) => {
                    c.shared.wait_extender.cancel().ok();
                    c.shared
                        .wait_extender
                        .start(Microseconds::try_from(milliseconds).unwrap());
                }
            }
            info!("{}-{}]", _starttime, perf_timer.elapsed().0 / 100);

            perf_timer.cancel().ok();
            perf_timer.start(60_000_000.microseconds());
        })
    }

    #[task(binds = ADC0, shared = [clock_ctrl], priority = 8)]
    fn adc_int(c: adc_int::Context) {
        c.shared.clock_ctrl.as_mut().unwrap().handle();
    }
}
