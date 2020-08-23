#![no_std]
use core::time::Duration;
use trussed_board::timer;
use lpc55_hal as hal;

use hal::traits::wg::timer::CountDown;
use hal::drivers::timer::Lap;

use hal::typestates::init_state::Enabled;
use hal::drivers::Timer as HalTimer;
use hal::peripherals::ctimer;
use hal::time::*;

/// A timer based on a lpc55 hal Ctimer.
///
/// Example:
///
/// ```rust
/// let bsp_timer1 = Timer::new(hal.ctimer.1.enabled(&mut hal.syscon));
/// let bsp_timer2 = Timer::new(hal.ctimer.2.enabled(&mut hal.syscon));
///
/// bsp_timer1.start(Duration::from_millis(10_000))
/// bsp_timer2.start(Duration::from_millis(1_000))
/// block!(bsp_timer2.wait());
/// let time_elapsed = bsp_timer1.lap().unwrap();
/// heprintln!("1 second ~ {} us", time_elapsed.as_micros());
/// // 1 second ~ 100056 us
/// ```

pub struct Timer<CTIMER>
where CTIMER: ctimer::Ctimer<Enabled>
{
    timer: HalTimer<CTIMER>
}

impl <CTIMER> Timer <CTIMER>
where CTIMER: ctimer::Ctimer<Enabled>
{
    pub fn new(timer: CTIMER) -> Self {
        Self {
            timer: HalTimer::new(timer),
        }
    }
}

impl <CTIMER> timer::Timer for Timer <CTIMER>
where CTIMER: ctimer::Ctimer<Enabled>
{
    /// Starts the timer.  It will run for the input duration.
    fn start(&mut self, count: Duration){
        self.timer.start((count.as_micros() as u32).us());
    }

    /// Read the current time elapsed since `start(...)` was last called.
    /// The time returned is only valid is the total running time hasn't elapsed yet.
    /// This returns an error if the running time has elapsed.
    fn lap(&mut self) -> nb::Result<Duration, timer::Error> {
        if ! self.timer.wait().is_ok() {
            Ok(Duration::from_micros(self.timer.lap().0 as u64))
        } else {
            Err(nb::Error::Other(timer::Error::TimerCompleted))
        }
    }

    /// Nonblockingly wait until the timer running duration has elapsed.
    fn wait(&mut self) -> nb::Result<(), core::convert::Infallible> {
        if self.timer.wait().is_ok() {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}