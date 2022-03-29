#![no_std]
#![no_main]

use embedded_runner_lib as ERL;
use nrf52840_hal::{
	gpio::{p0, p1},
	gpiote::Gpiote,
	rng::Rng,
	rtc::Rtc,
	timer::Timer,
};
use panic_halt as _;
use rand_core::SeedableRng;

#[macro_use]
extern crate delog;
delog::generate_macros!();

delog!(Delogger, 3*1024, 512, ERL::types::DelogFlusher);

#[rtic::app(device = nrf52840_hal::pac, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
        struct Resources {
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
		gpiote: Gpiote,
		power: nrf52840_pac::POWER,
		rtc: Rtc<nrf52840_pac::RTC0>,

		/* LPC55 specific elements */
		// perf_timer
		// clock_ctrl
		// wait_extender
	}

        #[init()]
        fn init(mut ctx: init::Context) -> init::LateResources {
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
		let dummy_ui = ERL::soc::dummy_ui::DummyUI::new();

		let platform: ERL::types::RunnerPlatform = ERL::types::RunnerPlatform::new(
			chacha_rng, store, dummy_ui);

		let mut trussed_service = trussed::service::Service::new(platform);

		let apps = ERL::init_apps(&mut trussed_service, &store, !powered_by_usb);

		let mut dev_rtc = Rtc::new(ctx.device.RTC0, 4095).unwrap();
		dev_rtc.enable_interrupt(nrf52840_hal::rtc::RtcInterrupt::Tick, None);
		dev_rtc.clear_counter();
		dev_rtc.enable_counter();

		// compose LateResources
		init::LateResources {
			trussed: trussed_service,
			apps,
			apdu_dispatch: usbnfcinit.apdu_dispatch,
			ctaphid_dispatch: usbnfcinit.ctaphid_dispatch,
			usb_classes: usbnfcinit.usb_classes,
			contactless: usbnfcinit.iso14443,
			gpiote: dev_gpiote,
			power: ctx.device.POWER,
			rtc: dev_rtc,
		}
	}

	#[idle(resources = [apps, apdu_dispatch, ctaphid_dispatch, usb_classes])]
	fn idle(ctx: idle::Context) -> ! {
		let idle::Resources { apps, apdu_dispatch, ctaphid_dispatch, mut usb_classes } = ctx.resources;

		trace!("idle");
		// TODO: figure out whether entering WFI is really worth it
		// cortex_m::asm::wfi();

		loop {
			Delogger::flush();

			let (usb_activity, _nfc_activity) =
				ERL::runtime::poll_dispatchers(apdu_dispatch, ctaphid_dispatch, apps);
			if usb_activity {
				trace!("app->usb");
				rtic::pend(nrf52840_pac::Interrupt::USBD);
			}
			// TODO: handle _nfc_activity

			let (_ccid_busy, _ctaphid_busy) = usb_classes.lock(
				|usb_classes| ERL::runtime::poll_usb_classes(usb_classes)
			);
			// TODO: kick off wait extensions
		}
		// loop {}
	}

	#[task(priority = 2, binds = SWI0_EGU0, resources = [trussed])]
	fn task_trussed(ctx: task_trussed::Context) {
		trace!("irq SWI0_EGU0");
		ERL::runtime::run_trussed(ctx.resources.trussed);
	}

	#[task(priority = 2, binds = GPIOTE, resources = [gpiote])] /* ui, fpr */
	fn task_button_irq(_ctx: task_button_irq::Context) {
		trace!("irq GPIOTE");
	}

        #[task(priority = 3, binds = USBD, resources = [usb_classes])]
        fn task_usb(ctx: task_usb::Context) {
		// trace!("irq USB");
		let usb_classes = ctx.resources.usb_classes;

		let (_ccid_busy, _ctaphid_busy) = ERL::runtime::poll_usb_classes(usb_classes);
		// TODO: kick off wait extensions
	}

	/* TODO: implement ctaphid_keepalive(), ccid_keepalive(), nfc_keepalive() */

        #[task(priority = 3, binds = RTC0, resources = [rtc], schedule = [foo])]
        fn task_rtc(ctx: task_rtc::Context) {
		trace!("irq RTC");
		ctx.resources.rtc.reset_event(nrf52840_hal::rtc::RtcInterrupt::Tick);
	}

	#[task()]
	fn foo(_ctx: foo::Context) {}
/*
        #[task(priority = 3, binds = POWER_CLOCK, resources = [power], spawn = [frontend, late_setup_usb])]
        fn power_handler(ctx: power_handler::Context) {}
**/

	extern "C" {
		fn SWI4_EGU4();
		// fn SWI5_EGU5();
	}
};
