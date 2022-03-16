#![no_std]
#![no_main]

use embedded_runner_lib::{
	self as ERL,
	types::BootMode
};
use embedded_hal::digital::v2::InputPin;
use embedded_time::rate::Megahertz;
use panic_halt as _;

#[macro_use]
extern crate delog;
delog::generate_macros!();

delog!(Delogger, 3*1024, 512, ERL::types::DelogFlusher);

#[rtic::app(device = lpc55_hal::raw, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
        struct Resources {
		// trussed: ERL::types::Trussed,
		// apps: ERL::types::Apps,
		apdu_dispatch: ERL::types::ApduDispatch,
		ctaphid_dispatch: ERL::types::CtaphidDispatch,
		usb_classes: Option<ERL::types::usb::UsbClasses>,
		// contactless: Option<ERL::types::Iso14443>,
		boot_mode: BootMode,

		/* LPC55 specific elements */
		v: u32,
		// perf_timer
		// clock_ctrl
		// wait_extender
	}

        #[init()]
        fn init(mut ctx: init::Context) -> init::LateResources {
		rtt_target::rtt_init_print!();
		Delogger::init_default(delog::LevelFilter::Debug, &ERL::types::DELOG_FLUSHER).ok();
		ERL::banner();

		ERL::soc::init_bootup(&mut ctx.device.IOCON);

		let mut hal = lpc55_hal::Peripherals::from((ctx.device, ctx.core));
		let (anactrl, pmc, syscon) = (
			&mut hal.anactrl,
			&mut hal.pmc,
			&mut hal.syscon);

		let iocon = &mut hal.iocon.enabled(syscon);
		let gpio = &mut hal.gpio.enabled(syscon);
		let nfc_irq = lpc55_hal::drivers::pins::Pio0_19::take().unwrap().into_gpio_pin(iocon, gpio).into_input();
		let bootmode = if nfc_irq.is_low().ok().unwrap() { BootMode::NFCPassive } else { BootMode::Full };

		// GPIO

		/* check reason for booting */
		/* a) powered through NFC: enable NFC, keep external oscillator off, don't start USB */
		/* b) powered through USB: start external oscillator, start USB, keep NFC off(?) */

		/* initializer::initialize_all() */
		/* -> initializer::initialize_clocks() */
		let clockfreq = if bootmode == BootMode::NFCPassive { Megahertz(4_u32) } else { Megahertz(96_u32) };
		let mut clocks = lpc55_hal::ClockRequirements::default().system_frequency(clockfreq)
					.configure(anactrl, pmc, syscon)
					.expect("LPC55 Clock Configuration Failed");

		let mut delay_timer = lpc55_hal::drivers::Timer::new(hal.ctimer.0.enabled(syscon, clocks.support_1mhz_fro_token().unwrap()));
		let mut perf_timer = lpc55_hal::drivers::Timer::new(hal.ctimer.4.enabled(syscon, clocks.support_1mhz_fro_token().unwrap()));
		// out: { nfc_irq, clocks, iocon, gpio }

		/* -> initializer::initialize_basic() */
		let _adc = lpc55_hal::Adc::from(hal.adc)
					.configure(ERL::soc::clock_controller::DynamicClockController::adc_configuration())
					.enabled(pmc, syscon);

		let _rgb = ERL::soc::init_rgb(syscon, iocon, hal.ctimer.3, &mut clocks);
		// let _buttons = ...;
		// check CFPA
		// BOOTROM check
		// out: { delay_timer, perf_timer, pfr, adc, buttons, rgb }

		/* -> initializer::initialize_usb() */
		let usbd_ref = { if bootmode == BootMode::Full {
			#[cfg(feature = "usbfs-peripheral")]
			{ Some(ERL::soc::setup_usb_bus(hal.usbfs, anactrl, iocon, pmc, syscon, clocks, &mut delay_timer)) }
			#[cfg(not(feature = "usbfs-peripheral"))]
			{ Some(ERL::soc::setup_usb_bus(hal.usbhs, anactrl, iocon, pmc, syscon, clocks, &mut delay_timer)) }
		} else {
			None
		}};
		// out: { usb_classes, contact_responder, ctaphid_responder }

		/* -> initializer::initialize_nfc() */
		let nfc_dev = { if bootmode == BootMode::NFCPassive {
			ERL::soc::setup_fm11nc08(&clocks, syscon, iocon, gpio,
					hal.flexcomm.0, hal.inputmux, hal.pint, nfc_irq, &mut delay_timer)
		} else {
			None
		}};
		// out: { iso14443, contactless_responder }

		/* -> initializer::initialize_interfaces() */
		let usbnfcinit = ERL::init_usb_nfc(usbd_ref, nfc_dev);
		// out: { apdu_dispatch, ctaphid_dispatch }

		/* -> initializer::initialize_flash() */
		// out: { flash_gordon, prince, rng }

		/* -> initializer::initialize_filesystem() */
		// out: { store, internal_storage_fs }

		/* -> initializer::initialize_trussed() */
		// out: trussed

		// let usbinit = ERL::init_usb( unsafe { USBD.as_ref().unwrap() });

		/*let internal_flash = ERL::soc::init_internal_flash(ctx.device.NVMC);
		let external_flash = {
			let dev_spim3 = Spim::new(ctx.device.SPIM3,
				board_gpio.flashnfc_spi.take().unwrap(),
				nrf52840_hal::spim::Frequency::M2,
				nrf52840_hal::spim::MODE_0,
				0x00u8
			);
			ERL::soc::init_external_flash(dev_spim3,
				board_gpio.flash_cs.take().unwrap(),
				board_gpio.flash_power
			)
		};*/
		// let store: ERL::types::RunnerStore = ERL::init_store(internal_flash, external_flash);

		// let dev_rng = Rng::new(ctx.device.RNG);
		// let chacha_rng = chacha20::ChaCha8Rng::from_rng(dev_rng).unwrap();
		// let dummy_ui = ERL::soc::dummy_ui::DummyUI::new();

		// let platform: ERL::types::RunnerPlatform = ERL::types::RunnerPlatform::new(
			// chacha_rng, store, dummy_ui);

		// let mut trussed_service = trussed::service::Service::new(platform);

		/*let apps = ERL::init_apps(&mut trussed_service, &store, powered?); */

		// compose LateResources
		init::LateResources {
			//trussed: trussed_service,
			//apps,
			apdu_dispatch: usbnfcinit.apdu_dispatch,
			ctaphid_dispatch: usbnfcinit.ctaphid_dispatch,
			usb_classes: usbnfcinit.usb_classes,
			//contactless: None,
			boot_mode: bootmode,

			//gpiote: dev_gpiote,
			//power: ctx.device.POWER,
			//rtc: dev_rtc,
			v: 5
		}
	}

	#[idle()]
	fn idle(_ctx: idle::Context) -> ! {
		/*
		   Note: ARM SysTick stops in WFI. This is unfortunate as
		   - RTIC uses SysTick for its schedule() feature
		   - we would really like to use WFI in order to save power
		   In the future, we might even consider entering "System OFF".
		   In short, don't expect schedule() to work.
		*/
		loop {
			trace!("idle");
			Delogger::flush();
			cortex_m::asm::wfi();
		}
		// loop {}
	}

};
