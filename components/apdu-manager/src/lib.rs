#![cfg_attr(not(feature = "std"), no_std)]

pub mod traits;
pub use traits::{
    AppletResponse,
    Applet,
    Aid,
    Result,
    ScratchBuffer,
};

pub mod manager;
pub use manager::{
    ApduManager,
};

pub mod types;