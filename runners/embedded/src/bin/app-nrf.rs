#![no_std]
#![no_main]

use embedded_runner_lib as ERL;
use nrf52840_hal::{
	gpio::{p0, p1},
	gpiote::Gpiote,
	spim::Spim,
};
use panic_halt as _;

#[macro_use]
extern crate delog;
delog::generate_macros!();

delog!(Delogger, 3*1024, 512, ERL::types::DelogFlusher);

#[rtic::app(device = nrf52840_hal::pac, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
        struct Resources {
		apdu_dispatch: ERL::types::ApduDispatch,
		ctaphid_dispatch: ERL::types::CtaphidDispatch,
		trussed: ERL::types::Trussed,
		apps: ERL::types::Apps,
		usb_classes: Option<ERL::types::UsbClasses>,
		contactless: Option<ERL::types::Iso14443>,

		/* NRF specific elements */
		// (display UI)
		// (fingerprint sensor)
		// (SE050)
		/* NRF specific device peripherals */
		// gpiote
		// power
		// rtc

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
		Delogger::init_default(delog::LevelFilter::Debug, &ERL::types::DELOG_FLUSHER).ok();
		info_now!("Embedded Runner (NRF) using librunner {}.{}.{}",
			ERL::types::build_constants::CARGO_PKG_VERSION_MAJOR,
			ERL::types::build_constants::CARGO_PKG_VERSION_MINOR,
			ERL::types::build_constants::CARGO_PKG_VERSION_PATCH);

		ERL::soc::board::init_bootup(&ctx.device.FICR, &ctx.device.UICR, &mut ctx.device.POWER);

		let dev_gpiote = Gpiote::new(ctx.device.GPIOTE);
		let board_gpio = {
			let dev_gpio_p0 = p0::Parts::new(ctx.device.P0);
			let dev_gpio_p1 = p1::Parts::new(ctx.device.P1);
			ERL::soc::board::init_pins(&dev_gpiote, dev_gpio_p0, dev_gpio_p1)
		};
		dev_gpiote.reset_events();

		// - usb (SoC)
		// - nfc (SoC)
		// - interfaces, generic USB machinery... (indep)

		let internal_flash = ERL::soc::init_internal_flash(ctx.device.NVMC);
		let _external_flash = {
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
		};

		/* see src/soc_nrf52840/types.rs for the reason why we cannot use
		   our lovingly crafted ExternalFlashStorage... */
		let store: ERL::types::RunnerStore = ERL::init_store(internal_flash, ERL::soc::types::ExternalStorage::new());

		// - trussed (indep)
		// - apps (indep)

		// compose LateResources
		init::LateResources { }
	}

};
