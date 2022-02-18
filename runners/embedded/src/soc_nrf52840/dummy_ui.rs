use trussed::platform::{consent, reboot, ui};

pub struct DummyUI {}

impl DummyUI {
	pub fn new() -> Self { Self {} }
}

impl trussed::platform::UserInterface for DummyUI {
	fn check_user_presence(&mut self) -> consent::Level {
		consent::Level::None
	}

	fn set_status(&mut self, _status: ui::Status) {
		info!("UI SetStatus: {:?}", _status);
	}

	fn refresh(&mut self) {}

	fn uptime(&mut self) -> core::time::Duration {
		let cyccnt = cortex_m::peripheral::DWT::cycle_count();
		core::time::Duration::new((cyccnt as u64) / 32_000, (cyccnt / 32) % 1_000)
	}

	fn reboot(&mut self, _to: reboot::To) -> ! {
		cortex_m::peripheral::SCB::sys_reset();
	}
}
