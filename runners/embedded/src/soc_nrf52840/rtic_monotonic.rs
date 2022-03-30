use nrf52840_hal::rtc::{Rtc, RtcCompareReg, RtcInterrupt};
type Rtc0 = Rtc<nrf52840_pac::RTC0>;

const RTC_HZ: u64 = 200;

#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct RtcInstant(u64);
impl RtcInstant {
	fn wrapped(&self) -> u32 { (self.0 >> 24) as u32 }
	fn cnt(&self) -> u32 { (self.0 as u32) & 0xffffff_u32 }
}
impl From<RtcInstant> for embedded_time::duration::units::Milliseconds {
	fn from(i: RtcInstant) -> Self {
		Self( ((i.0 * 1000) / RTC_HZ) as u32 )
	}
}

#[derive(Copy, Clone)]
pub struct RtcDuration(u64);
impl RtcDuration {
	pub fn from_ms(ms: u32) -> Self {
		RtcDuration(((ms as u64) * RTC_HZ) / 1000)
	}
}
impl From<embedded_time::duration::units::Milliseconds> for RtcDuration {
	fn from(ms: embedded_time::duration::units::Milliseconds) -> Self {
		Self::from_ms(ms.0)
	}
}

impl core::ops::Sub for RtcInstant {
	type Output = RtcDuration;
	fn sub(self, other: Self) -> RtcDuration { RtcDuration(self.0 - other.0) }
}
impl core::ops::Add<RtcDuration> for RtcInstant {
	type Output = Self;
	fn add(self, other: RtcDuration) -> Self { Self(self.0 + other.0) }
}
impl core::ops::Sub<RtcDuration> for RtcInstant {
	type Output = Self;
	fn sub(self, other: RtcDuration) -> Self { Self(self.0 - other.0) }
}

pub struct RtcMonotonic {
	rtc: Rtc0,
	wrapped: u32,
}
impl RtcMonotonic {
	pub fn new(rtc_pac: nrf52840_pac::RTC0) -> Self {
		Self { rtc: Rtc::new(rtc_pac, 163).ok().unwrap(),
			wrapped: 0 }
	}
}

impl rtic::Monotonic for RtcMonotonic {
	type Instant = RtcInstant;
	type Duration = RtcDuration;

	fn zero() -> Self::Instant {
		RtcInstant(0u64)
	}

	fn now(&mut self) -> Self::Instant {
		let cnt: u32 = self.rtc.get_counter();
		/* we might be called from the RTC interrupt with the overflow event
		   still pending (our .on_interrupt() is called at the end of the handler) */
		let wrapped: u64 = if self.rtc.is_event_triggered(RtcInterrupt::Overflow) {
			(self.wrapped as u64) + 1
		} else {
			self.wrapped as u64
		};
		RtcInstant( (wrapped << 24) | cnt as u64 )
	}

	unsafe fn reset(&mut self) {
		self.rtc.clear_counter();
		self.rtc.enable_counter();
		self.rtc.reset_event(RtcInterrupt::Overflow);
		self.rtc.reset_event(RtcInterrupt::Compare0);
		self.rtc.enable_interrupt(RtcInterrupt::Compare0, None);
		self.rtc.enable_event(RtcInterrupt::Compare0);
		self.rtc.enable_interrupt(RtcInterrupt::Overflow, None);
		self.rtc.enable_event(RtcInterrupt::Overflow);
	}

	fn on_interrupt(&mut self) {
		if self.rtc.is_event_triggered(RtcInterrupt::Overflow) {
			self.wrapped += 1;
			self.rtc.reset_event(RtcInterrupt::Overflow);
		}
	}

	fn set_compare(&mut self, i: Self::Instant) {
		let now = self.now();

		/* RTIC uses us as a oneshot timer and reprograms us if we fire early */
		if now.wrapped() == i.wrapped() {
			self.rtc.set_compare(RtcCompareReg::Compare0, i.cnt()).ok();
		} else {
			/* instant is too far in the future; set CC[0] to fire at the
			   same time as the overflow, RTIC will reprogram us anyway */
			self.rtc.set_compare(RtcCompareReg::Compare0, 0).ok();
		}
	}

	fn clear_compare_flag(&mut self) {
		self.rtc.reset_event(RtcInterrupt::Compare0);
	}
}
