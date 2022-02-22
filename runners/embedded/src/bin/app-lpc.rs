#![no_std]
#![no_main]

use embedded_runner_lib as ERL;
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
		// apdu_dispatch: ERL::types::ApduDispatch,
		// ctaphid_dispatch: ERL::types::CtaphidDispatch,
		// usb_classes: Option<ERL::types::usb::UsbClasses>,
		// contactless: Option<ERL::types::Iso14443>,

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

		ERL::soc::init_bootup();

		// GPIO

		/* check reason for booting */
		/* a) powered through NFC: enable NFC, keep external oscillator off, don't start USB */
		/* b) powered through USB: start external oscillator, start USB, keep NFC off(?) */

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

		/*let apps = {
			#[cfg(feature = "provisioner-app")]
			{
				let store_2 = store.clone();
				let int_flash_ref = unsafe { ERL::types::INTERNAL_STORAGE.as_mut().unwrap() };
				let pnp = ERL::types::ProvisionerNonPortable {
					store: store_2,
					stolen_filesystem: int_flash_ref,
					nfc_powered: !powered_by_usb
				};
				ERL::types::Apps::new(&mut trussed_service, pnp)
			}
			#[cfg(not(feature = "provisioner-app"))]
			{ ERL::types::Apps::new(&mut trussed_service) }
		};*/

		//let dev_rtc = Rtc::new(ctx.device.RTC0, 4095).unwrap();

		// compose LateResources
		init::LateResources {
			//trussed: trussed_service,
			//apps,
			//apdu_dispatch: usbinit.apdu_dispatch,
			//ctaphid_dispatch: usbinit.ctaphid_dispatch,
			//usb_classes: Some(usbinit.classes),
			//contactless: None,
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
