//! This HAL now uses `embedded-time`.

pub use embedded_time::{
    duration::{Seconds, Milliseconds, Microseconds, Nanoseconds},
    rate::{
        Baud, Kilobaud, Megabaud,
        Hertz, Kilohertz, Megahertz,
    },
};

pub use embedded_time::duration::Extensions as DurationExtensions;
pub use embedded_time::rate::Extensions as RateExtensions;
