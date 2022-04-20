#![no_std]
#![no_main]

use embedded_runner_lib as ERL;

#[macro_use]
extern crate delog;
delog::generate_macros!();

delog!(Delogger, 3*1024, 512, ERL::types::DelogFlusher);

#[rtic::app(device = nrf52840_hal::pac, peripherals = true, dispatchers = [SWI3_EGU3, SWI4_EGU4, SWI5_EGU5])]
mod app {
	use super::{Delogger, ERL, ERL::soc::rtic_monotonic::RtcDuration};
	use nrf52840_hal::{
		gpio::{p0, p1},
		gpiote::Gpiote,
		rng::Rng,
		timer::Timer,
	};
	use panic_halt as _;
	use rand_core::SeedableRng;

	#[shared]
        struct SharedResources {
		trussed: ERL::types::Trussed,
		apps: ERL::types::Apps,
		apdu_dispatch: ERL::types::ApduDispatch,
		ctaphid_dispatch: ERL::types::CtaphidDispatch,
		usb_classes: Option<ERL::types::usbnfc::UsbClasses>,
		contactless: Option<ERL::types::Iso14443>,

		/* NRF specific elements */
		// (display UI)
		// (fingerprint sensor)
		// (SE050)
		/* NRF specific device peripherals */

		/* LPC55 specific elements */
		// perf_timer
		// clock_ctrl
		// wait_extender
	}

	#[local]
	struct LocalResources {
		gpiote: Gpiote,
		power: nrf52840_pac::POWER,
	}

	#[monotonic(binds = RTC0, default = true)]
	type RtcMonotonic = ERL::soc::rtic_monotonic::RtcMonotonic;

        #[init()]
        fn init(mut ctx: init::Context) -> (SharedResources, LocalResources, init::Monotonics) {
		ctx.core.DCB.enable_trace();
		ctx.core.DWT.enable_cycle_counter();

		rtt_target::rtt_init_print!();
		Delogger::init_default(delog::LevelFilter::Trace, &ERL::types::DELOG_FLUSHER).ok();
		ERL::banner();

		ERL::soc::init_bootup(&ctx.device.FICR, &ctx.device.UICR, &mut ctx.device.POWER);

		let mut delay_timer = Timer::<nrf52840_pac::TIMER0>::new(ctx.device.TIMER0);

		let dev_gpiote = Gpiote::new(ctx.device.GPIOTE);
		let mut board_gpio = {
			let dev_gpio_p0 = p0::Parts::new(ctx.device.P0);
			let dev_gpio_p1 = p1::Parts::new(ctx.device.P1);
			ERL::soc::board::init_pins(&dev_gpiote, dev_gpio_p0, dev_gpio_p1)
		};
		dev_gpiote.reset_events();

		/* check reason for booting */
		let powered_by_usb: bool = true;
		/* a) powered through NFC: enable NFC, keep external oscillator off, don't start USB */
		/* b) powered through USB: start external oscillator, start USB, keep NFC off(?) */

		let usbd_ref = { if powered_by_usb {
			Some(ERL::soc::setup_usb_bus(ctx.device.CLOCK, ctx.device.USBD))
		} else {
			None
		}};
		/* TODO: set up NFC chip */
		// let usbnfcinit = ERL::init_usb_nfc(usbd_ref, None);

		let internal_flash = ERL::soc::init_internal_flash(ctx.device.NVMC);

		#[cfg(feature = "extflash_qspi")]
		let extflash = {
			let mut qspi_extflash = ERL::soc::qspiflash::QspiFlash::new(ctx.device.QSPI,
				board_gpio.flashnfc_spi.take().unwrap(),
				board_gpio.flash_cs.take().unwrap(),
				board_gpio.flash_power,
				&mut delay_timer);
			qspi_extflash.activate();
			trace!("qspi jedec: {}", delog::hex_str!(&qspi_extflash.read_jedec_id()));
			use littlefs2::driver::Storage;
			let mut mybuf: [u8; 32] = [0u8; 32];
			mybuf[2] = 0x5a;
			qspi_extflash.read(0x400, &mut mybuf[0..16]).ok();
			trace!("qspi read: {}", delog::hex_str!(&mybuf[0..16]));

			qspi_extflash
		};
		#[cfg(not(feature = "extflash_qspi"))]
		let extflash = ERL::soc::types::ExternalStorage::new();

		let store: ERL::types::RunnerStore = ERL::init_store(internal_flash, extflash);

		let usbnfcinit = ERL::init_usb_nfc(usbd_ref, None);
		/* TODO: set up fingerprint device */
		/* TODO: set up SE050 device */
		/* TODO: set up display */

		let dev_rng = Rng::new(ctx.device.RNG);
		let chacha_rng = chacha20::ChaCha8Rng::from_rng(dev_rng).unwrap();

		#[cfg(feature = "board-nk3am")]
		let ui = ERL::soc::board::init_ui(board_gpio.rgb_led,
			ctx.device.PWM0, ctx.device.TIMER1,
			ctx.device.PWM1, ctx.device.TIMER2,
			ctx.device.PWM2, ctx.device.TIMER3,
			board_gpio.touch.unwrap(), delay_timer
		);

		#[cfg(not(feature = "board-nk3am"))]
		let ui = ERL::soc::board::init_ui();

		let platform: ERL::types::RunnerPlatform = ERL::types::RunnerPlatform::new(
			chacha_rng, store, ui);

		let mut trussed_service = trussed::service::Service::new(platform);

		let apps = ERL::init_apps(&mut trussed_service, &store, !powered_by_usb);

		let rtc_mono = RtcMonotonic::new(ctx.device.RTC0);

		ui::spawn_after(RtcDuration::from_ms(2500)).ok();

		// compose LateResources
		( SharedResources {
			trussed: trussed_service,
			apps,
			apdu_dispatch: usbnfcinit.apdu_dispatch,
			ctaphid_dispatch: usbnfcinit.ctaphid_dispatch,
			usb_classes: usbnfcinit.usb_classes,
			contactless: usbnfcinit.iso14443,
		}, LocalResources {
			gpiote: dev_gpiote,
			power: ctx.device.POWER,
		}, init::Monotonics(rtc_mono))
	}

	#[idle(shared = [apps, apdu_dispatch, ctaphid_dispatch, usb_classes, contactless])]
	fn idle(ctx: idle::Context) -> ! {
		let idle::SharedResources { mut apps, mut apdu_dispatch, mut ctaphid_dispatch, mut usb_classes, mut contactless } = ctx.shared;

		trace!("idle");
		// TODO: figure out whether entering WFI is really worth it
		// cortex_m::asm::wfi();

		loop {
			Delogger::flush();

			let (usb_activity, _nfc_activity) =
				apps.lock(|apps|
				apdu_dispatch.lock(|apdu_dispatch|
				ctaphid_dispatch.lock(|ctaphid_dispatch|
				ERL::runtime::poll_dispatchers(apdu_dispatch, ctaphid_dispatch, apps)
			)));
			if usb_activity {
				/*trace!("app->usb");*/
				rtic::pend(nrf52840_pac::Interrupt::USBD);
			}

			usb_classes.lock(|usb_classes| {
				ERL::runtime::poll_usb(usb_classes,
					ccid_keepalive::spawn_after,
					ctaphid_keepalive::spawn_after,
					monotonics::now().into());
			});

			contactless.lock(|contactless| {
				ERL::runtime::poll_nfc(contactless, nfc_keepalive::spawn_after);
			});
		}
		// loop {}
	}

	#[task(priority = 2, binds = SWI0_EGU0, shared = [trussed])]
	fn task_trussed(ctx: task_trussed::Context) {
		let mut trussed = ctx.shared.trussed;

		trace!("irq SWI0_EGU0");
		trussed.lock(|trussed| {
			ERL::runtime::run_trussed(trussed);
		});
	}

	#[task(priority = 5, binds = GPIOTE, local = [gpiote])] /* ui, fpr */
	fn task_button_irq(_ctx: task_button_irq::Context) {
		trace!("irq GPIOTE");
	}

	#[task(priority = 3, binds = USBD, shared = [usb_classes])]
	fn task_usb(ctx: task_usb::Context) {
		// trace!("irq USB");
		let mut usb_classes = ctx.shared.usb_classes;

		usb_classes.lock(|usb_classes| {
			ERL::runtime::poll_usb(usb_classes,
				ccid_keepalive::spawn_after,
				ctaphid_keepalive::spawn_after,
				monotonics::now().into());
		});
	}

	#[task(priority = 3, shared = [usb_classes])]
	fn ccid_keepalive(ctx: ccid_keepalive::Context) {
		let mut usb_classes = ctx.shared.usb_classes;

		usb_classes.lock(|usb_classes| {
			ERL::runtime::ccid_keepalive(usb_classes, ccid_keepalive::spawn_after);
		});
	}

	#[task(priority = 3, shared = [usb_classes])]
	fn ctaphid_keepalive(ctx: ctaphid_keepalive::Context) {
		let mut usb_classes = ctx.shared.usb_classes;

		usb_classes.lock(|usb_classes| {
			ERL::runtime::ctaphid_keepalive(usb_classes, ctaphid_keepalive::spawn_after);
		});
	}

	#[task(priority = 4, shared = [contactless])]
	fn nfc_keepalive(ctx: nfc_keepalive::Context) {
		let mut contactless = ctx.shared.contactless;

		contactless.lock(|contactless| {
			ERL::runtime::nfc_keepalive(contactless, nfc_keepalive::spawn_after);
		});
	}

	#[task(priority = 5, binds = POWER_CLOCK, local = [power])]
	fn power_handler(ctx: power_handler::Context) {
		let mut power = ctx.local.power;

		trace!("irq PWR {:x} {:x} {:x}",
			power.mainregstatus.read().bits(),
			power.usbregstatus.read().bits(),
			power.pofcon.read().bits());

		if power.events_usbdetected.read().events_usbdetected().bits() {
			power.events_usbdetected.write(|w| unsafe { w.bits(0) });
			trace!("usb+");
		}
		if power.events_usbpwrrdy.read().events_usbpwrrdy().bits() {
			power.events_usbpwrrdy.write(|w| unsafe { w.bits(0) });
			trace!("usbY");
		}
		if power.events_usbremoved.read().events_usbremoved().bits() {
			power.events_usbremoved.write(|w| unsafe { w.bits(0) });
			trace!("usb-");
		}
	}

	#[task(priority = 1)]
	fn ui(_ctx: ui::Context) {
		trace!("UI");
		ui::spawn_after(RtcDuration::from_ms(2500)).ok();
	}
}
