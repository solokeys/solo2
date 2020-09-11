use core::time::Duration;
use core::convert::Infallible;

/// Timer trait for Trussed, with inspiration from
/// https://docs.rs/embedded-hal/0.2.3/embedded_hal/timer/trait.CountDown.html
///

///
/// Usage Example:
///
/// ```rust
/// bsp_timer1.start(Duration::from_millis(10_000))
/// bsp_timer2.start(Duration::from_millis(1_000))
/// block!(bsp_timer2.wait());
/// let time_elapsed = bsp_timer1.lap().unwrap();
/// heprintln!("1 second = {} us", time_elapsed.as_micros());
/// ```

pub enum Error{
    TimerCompleted
}

pub trait Timer {
    /// Starts the timer.  It will run for the input duration.
    fn start(&mut self, count: Duration);

    /// Read the current time elapsed since `start(...)` was last called.
    /// The time returned is only valid is the total running time hasn't elapsed yet.
    /// This returns an error if the running time has elapsed.
    fn lap(&mut self) -> nb::Result<Duration, Error>;

    /// Nonblockingly wait until the timer running duration has elapsed.
    fn wait(&mut self) -> nb::Result<(), Infallible>;
}
