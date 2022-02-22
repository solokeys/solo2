// TODO: clean this up, *-imports are horrible
pub use crate::traits::wg::prelude::*;

// Uhh get rid of this as soon as Vadim did v3
pub use crate::traits::wg::digital::v2::OutputPin as _;
pub use crate::traits::wg::digital::v2::StatefulOutputPin as _;

pub use crate::time::DurationExtensions as _;
pub use crate::time::RateExtensions as _;

pub use crate::traits::flash::Read as _;
pub use crate::traits::flash::WriteErase as _;

// Good idea? Bad idea?
// Compare and contrast with renaming of above traits
pub use super::drivers::prelude::*;
