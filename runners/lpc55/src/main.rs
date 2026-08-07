//! main app in cortex-m-rtic version
//!
//! See also `main_rt.rs` for a RT-only version.

#![no_std]
#![no_main]
// #![deny(warnings)]

const REFRESH_MILLISECS: u32 = 50;

const USB_INTERRUPT: board::hal::raw::Interrupt = board::hal::raw::Interrupt::USB1;
const NFC_INTERRUPT: board::hal::raw::Interrupt = board::hal::raw::Interrupt::PIN_INT0;

use defmt_rtt as _;

use core::arch::asm;

#[inline]
pub fn msp() -> u32 {
    let r;
    unsafe { asm!("mrs {}, MSP", out(reg) r, options(nomem, nostack, preserves_flags)) };
    r
}

#[rtic::app(device = runner::hal::raw, peripherals = true, dispatchers = [PLU, PIN_INT5, PIN_INT7])]
mod app {
    #[allow(
        unused,
        reason = "Actual calls are only generated when logging is enabled."
    )]
    use super::msp;
    use board::hal::time::Milliseconds;
    use board::CLOCK_FREQ;
    use defmt::{debug, info};
    use hal::drivers::timer::Elapsed;
    use hal::time::{DurationExtensions, Microseconds};
    use hal::traits::wg::timer::Cancel;
    use hal::traits::wg::timer::CountDown;
    use rtic_sync::channel::{Receiver, Sender};
    use rtic_sync::make_channel;
    use runner::hal;

    use rtic_monotonics::systick::prelude::*;

    use crate::{NFC_INTERRUPT, REFRESH_MILLISECS, USB_INTERRUPT};

    systick_monotonic!(Mono, 1000);

    #[local]
    struct LocalResources {
        updates: u32,
        ccid_wait_extension_receiver: Receiver<'static, Milliseconds, 1>,
        ctaphid_keep_alive_receiver: Receiver<'static, Milliseconds, 1>,
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

        /// Wallet hardware-wallet app + its HID dispatch. Kept off the
        /// `apps` lock so its blocking sign path doesn't stall other apps.
        /// `WalletSlot` is `Option<Wallet>` with the `wallet` feature on and
        /// `()` off, so this resource is always present for RTIC.
        wallet: runner::types::WalletSlot,

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

        /// Used for scheduling sending of ccid wait extensions.
        ccid_wait_extension_sender: Sender<'static, Milliseconds, 1>,

        /// Used for scheduling ctaphid keep alive messages.
        ctaphid_keep_alive_sender: Sender<'static, Milliseconds, 1>,
    }

    #[init]
    fn init(c: init::Context) -> (SharedResources, LocalResources) {
        // Bootloop trap: if the previous reset came from the watchdog, jump
        // to MBoot before running init_board so the device stays recoverable.
        // The wdtreset bit (PMC.aoreg1) only auto-clears on POR/BOD, so we
        // clear it eagerly — a single MBoot recovery flash returns the next
        // reset to normal operation.
        //
        // This build does not arm WWDT, but we keep the trap defensively: a
        // prior firmware on the same device might have armed it and fired.
        let wdt_caused_reset = c.device.PMC.aoreg1.read().wdtreset().bit();
        if wdt_caused_reset {
            c.device.PMC.aoreg1.modify(|_, w| w.wdtreset().clear_bit());
            runner::hal::boot_to_bootrom();
        }

        let (
            apdu_dispatch,
            ctaphid_dispatch,
            trussed,
            apps,
            wallet,
            usb_classes,
            contactless,
            perf_timer,
            clock_ctrl,
            wait_extender,
        ) = runner::init_board(c.device);

        Mono::start(c.core.SYST, CLOCK_FREQ);

        ccid_wait_extension::spawn().unwrap();
        ctaphid_keepalive::spawn().unwrap();

        // don't toggle LED in passive mode
        if usb_classes.is_some() {
            hal::enable_cycle_counter();
            // c.schedule.update_ui(Instant::now() + PERIOD.cycles()).unwrap();
            update_ui::spawn().unwrap();
        }

        let (ccid_wait_extension_sender, ccid_wait_extension_receiver) =
            make_channel!(Milliseconds, 1);
        let (ctaphid_keep_alive_sender, ctaphid_keep_alive_receiver) =
            make_channel!(Milliseconds, 1);

        (
            SharedResources {
                apdu_dispatch,
                ctaphid_dispatch,
                trussed,

                apps,

                wallet,

                usb_classes,
                contactless,

                perf_timer,

                clock_ctrl,
                wait_extender,

                ccid_wait_extension_sender,
                ctaphid_keep_alive_sender,
            },
            LocalResources {
                updates: 1,
                ccid_wait_extension_receiver,
                ctaphid_keep_alive_receiver,
            },
        )
    }

    #[idle(shared = [apdu_dispatch, ctaphid_dispatch, apps, perf_timer, usb_classes, ccid_wait_extension_sender, ctaphid_keep_alive_sender, wallet])]
    fn idle(mut c: idle::Context) -> ! {
        info!("inside IDLE, initial SP = {:08X}", msp());
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

            let apdu_result = c
                .shared
                .apps
                .apdu_dispatch(|apps| c.shared.apdu_dispatch.poll(apps));
            match apdu_result {
                Some(apdu_dispatch::iso7816::Interface::Contact) => {
                    rtic::pend(USB_INTERRUPT);
                }
                Some(apdu_dispatch::iso7816::Interface::Contactless) => {
                    rtic::pend(NFC_INTERRUPT);
                }
                _ => {}
            }

            c.shared.usb_classes.lock(|usb_classes_maybe| {
                if usb_classes_maybe.is_some() {
                    let usb_classes = usb_classes_maybe.as_mut().unwrap();

                    // Poll USB first so data is available before dispatching
                    usb_classes.poll();

                    usb_classes.ctaphid.check_timeout(time / 1000);

                    if let usbd_ccid::Status::ReceivedData(milliseconds) =
                        usb_classes.ccid.did_start_processing()
                    {
                        c.shared
                            .ccid_wait_extension_sender
                            .lock(|ccid_wait_extension_sender| {
                                ccid_wait_extension_sender.try_send(milliseconds)
                            })
                            .ok();
                    }
                    if let usbd_ctaphid::types::Status::ReceivedData(milliseconds) =
                        usb_classes.ctaphid.did_start_processing()
                    {
                        c.shared
                            .ctaphid_keep_alive_sender
                            .lock(|ctaphid_keep_alive_sender| {
                                ctaphid_keep_alive_sender.try_send(milliseconds)
                            })
                            .ok();
                    }
                }
            });

            // Poll CTAP HID dispatch OUTSIDE the usb_classes lock.
            // ctaphid_dispatch.poll() may trigger a trussed syscall which pends OS_EVENT.
            // If called inside usb_classes.lock(), the resulting critical section would
            // prevent OS_EVENT from running, causing a deadlock.
            if c.shared
                .apps
                .ctaphid_dispatch(|apps| c.shared.ctaphid_dispatch.poll(apps))
            {
                rtic::pend(USB_INTERRUPT);
            }

            // Fill the wallet consent result from the idle-reachable inputs
            // (monotonic clock + NFC field) — the non-blocking UP check. Uses the
            // systick `Mono` (real 1 kHz ms), not `perf_timer` (a CTIMER not
            // calibrated to real ms), so the 30 s consent timeout is accurate.
            #[cfg(feature = "wallet")]
            runner::confirm_user_present_non_blocking(
                Mono::now().duration_since_epoch().to_millis(),
            );

            // Mirror a waiting wallet sign into the LED driver so the UP
            // indicator lights during a wallet sign (the wallet path never
            // calls trussed's `set_status`). The board-crate LED driver
            // (`update_ui` task) ORs this with trussed's own status.
            #[cfg(feature = "wallet")]
            board::trussed::set_wallet_up_requested(wallet_app::consent::is_up_requested());

            // Drive the wallet HID transport. Polled outside the
            // `usb_classes`/`apps` locks because a sign-message doesn't block;
            // pend USB if it produced a response.
            #[cfg(feature = "wallet")]
            {
                let pending = c
                    .shared
                    .wallet
                    .lock(|wallet| wallet.as_mut().map(|s| s.poll()).unwrap_or(false));
                if pending {
                    rtic::pend(USB_INTERRUPT);
                }
            }
        }
    }

    #[task(binds = USB1_NEEDCLK, priority=6)]
    fn usb1_needclk(_c: usb1_needclk::Context) {
        // Behavior is same as in USB1 handler
        rtic::pend(USB_INTERRUPT);
    }

    /// Manages all traffic on the USB bus.
    #[task(binds = USB1, shared = [usb_classes, ccid_wait_extension_sender, ctaphid_keep_alive_sender], priority=6)]
    fn usb(mut c: usb::Context) {
        let usb = unsafe { hal::raw::Peripherals::steal().USB1 };
        let intstat = usb.intstat.read().bits();
        // Log non-SOF USB interrupts (SOF = bit 30, dev_status = bit 31)
        if intstat & 0x0FFF_FFFF != 0 {
            defmt::debug!("USB interrupt: intstat={:08x}", intstat);
        }
        c.shared.usb_classes.lock(|usb_classes_maybe| {
            let usb_classes = usb_classes_maybe.as_mut().unwrap();

            usb_classes.poll();

            if let usbd_ccid::Status::ReceivedData(milliseconds) =
                usb_classes.ccid.did_start_processing()
            {
                c.shared
                    .ccid_wait_extension_sender
                    .lock(|ccid_wait_extension_sender| {
                        ccid_wait_extension_sender.try_send(milliseconds)
                    })
                    .ok();
            }
            if let usbd_ctaphid::types::Status::ReceivedData(milliseconds) =
                usb_classes.ctaphid.did_start_processing()
            {
                c.shared
                    .ctaphid_keep_alive_sender
                    .lock(|ctaphid_keep_alive_sender| {
                        ctaphid_keep_alive_sender.try_send(milliseconds)
                    })
                    .ok();
            }
        });

        // let after = Instant::now();
        // let length = (after - before).as_cycles();
        // if length > 10_000 {
        //     // debug!("poll took {:?} cycles", length);
        // }
        // if remaining < 60_000 {
        //     debug_now!("USB interrupt done: {} bytes", remaining);
        // }
    }

    /// Whenever we start waiting for an application to reply to CCID, this must be scheduled.
    /// In case the application takes too long, this will periodically send wait extensions
    /// until the application replied.
    #[task(shared = [usb_classes, ccid_wait_extension_sender], local = [ccid_wait_extension_receiver], priority = 6)]
    async fn ccid_wait_extension(mut c: ccid_wait_extension::Context) {
        loop {
            let milliseconds = c.local.ccid_wait_extension_receiver.recv().await.unwrap();
            Mono::delay(milliseconds.0.millis()).await;
            debug!("CCID WAIT EXTENSION");
            debug!("remaining stack size: {} bytes", msp() - 0x2000_0000);
            let status = c.shared.usb_classes.lock(|usb_classes_maybe| {
                usb_classes_maybe
                    .as_mut()
                    .unwrap()
                    .ccid
                    .send_wait_extension()
            });
            c.shared
                .ccid_wait_extension_sender
                .lock(|ccid_wait_extension_sender| {
                    c.local.ccid_wait_extension_receiver.try_recv().ok();
                    if let usbd_ccid::Status::ReceivedData(milliseconds) = status {
                        ccid_wait_extension_sender.try_send(milliseconds).ok();
                    }
                });
        }
    }

    /// Same as with CCID, but sending ctaphid keepalive statuses.
    #[task(shared = [usb_classes, ctaphid_keep_alive_sender], local = [ctaphid_keep_alive_receiver], priority = 6)]
    async fn ctaphid_keepalive(mut c: ctaphid_keepalive::Context) {
        loop {
            let milliseconds = c.local.ctaphid_keep_alive_receiver.recv().await.unwrap();
            Mono::delay(milliseconds.0.millis()).await;
            debug!("CTAPHID keepalive");
            debug!("remaining stack size: {} bytes", msp() - 0x2000_0000);
            let status = c.shared.usb_classes.lock(|usb_classes_maybe| {
                usb_classes_maybe
                    .as_mut()
                    .unwrap()
                    .ctaphid
                    .send_keepalive(board::trussed::UserPresenceStatus::waiting())
            });
            c.shared
                .ctaphid_keep_alive_sender
                .lock(|ctaphid_keep_alive_sender| {
                    c.local.ctaphid_keep_alive_receiver.try_recv().ok();
                    if let usbd_ctaphid::types::Status::ReceivedData(milliseconds) = status {
                        ctaphid_keep_alive_sender.try_send(milliseconds).ok();
                    }
                });
        }
    }

    #[task(binds = OS_EVENT, shared = [trussed], priority = 5)]
    fn os_event(mut c: os_event::Context) {
        // debug_now!("os event: remaining stack size: {} bytes", msp() - 0x2000_0000);
        c.shared.trussed.lock(|trussed| trussed.process());
    }

    #[task(shared = [trussed], local = [updates], priority = 1)]
    async fn update_ui(mut c: update_ui::Context) {
        loop {
            Mono::delay(REFRESH_MILLISECS.millis()).await;
            // debug_now!("update UI: remaining stack size: {} bytes", msp() - 0x2000_0000);

            // let wait_periods = c.resources.trussed.lock(|trussed| trussed.update_ui());
            c.shared.trussed.lock(|trussed| trussed.update_ui());
            // c.schedule.update_ui(Instant::now() + wait_periods * PERIOD.cycles()).unwrap();

            *c.local.updates += 1;
        }
    }

    #[task(binds = CTIMER0, shared = [contactless, perf_timer, wait_extender], priority = 7)]
    fn nfc_wait_extension(mut c: nfc_wait_extension::Context) {
        c.shared.perf_timer.lock(|_perf_timer| {
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
        });
    }

    #[task(binds = PIN_INT0, shared = [
            contactless, perf_timer, wait_extender,
        ], priority = 7,
    )]
    fn nfc_irq(mut c: nfc_irq::Context) {
        c.shared.perf_timer.lock(|perf_timer| {
            // No contactless frontend (USB-only boot, or NFC chip absent): the apdu
            // dispatch loop still pends NFC_INTERRUPT, so bail instead of panicking.
            let Some(contactless) = c.shared.contactless.as_mut() else {
                return;
            };
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
