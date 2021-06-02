//! # Initialization stages for LPC55.
//! 
//! The structs here contain the items that get initialized.
//! Each struct is initialized sequentially, one after the other.
//! Each stage consumed the previous as a prerequisite.
//! 
//! If a peripheral is needed, it is included in the initialization process as late as possible.
//! - If a problem occurs, it is easier to recover the further into initialization it is (e.g. boot to bootloader).
//! - Other setups that do not need the full initialization can be more lean.
//! 
use crate::hal;
use hal::drivers::{
    clocks::Clocks,
    flash::FlashGordon,
    pins::direction,
    Timer
};
use hal::typestates::pin::state::Gpio;
use hal::peripherals::{
    prince::Prince,
};
use hal::peripherals::pfr::Pfr;
use crate::types;

/// Initialized clocks, Nfc interrupt pin, Iocon, Gpio.
pub struct Clock {
    pub clocks: Clocks,
    pub nfc_irq: Option<hal::Pin<board::nfc::NfcIrqPin, Gpio<direction::Input>>>,
    pub iocon: hal::Iocon<hal::Enabled>,
    pub gpio: hal::Gpio<hal::Enabled>,

    // prevent outside sources from constructing
    pub(crate) _clock: (),
}

/// Initialized delay & performance timers, Adc, Buttons, Nfc chip, RGB LED
pub struct Basic {
    pub delay_timer: Timer<hal::peripherals::ctimer::Ctimer0<hal::Enabled>>,
    pub perf_timer: Timer<hal::peripherals::ctimer::Ctimer4<hal::Enabled>>,
    pub pfr: Pfr<hal::Enabled>,

    pub adc: Option<hal::Adc<hal::Enabled>>,
    pub three_buttons: Option<board::ThreeButtons>,
    pub rgb: Option<board::RgbLed>,
}

/// Initialized NFC Iso14443 transport
pub struct Nfc {
    pub iso14443: Option<nfc_device::Iso14443<board::nfc::NfcChip>>,

    pub contactless_responder: Option<interchange::Responder<apdu_dispatch::interchanges::Contactless>>,
}

/// Initialized USB device + USB classes, Dynamic Clock controller.
pub struct Usb {
    pub usb_classes: Option<types::UsbClasses>,

    pub contact_responder: Option<interchange::Responder<apdu_dispatch::interchanges::Contact>>,
    pub ctaphid_responder: Option<interchange::Responder<ctaphid_dispatch::types::HidInterchange>>,
}

/// Initialized apdu + ctaphid dispatches
pub struct Interfaces {
    pub apdu_dispatch: types::ApduDispatch,
    pub ctaphid_dispatch: types::CtaphidDispatch,
}

/// Initialized flash driver, prince, RNG.
pub struct Flash {
    pub flash_gordon: Option<FlashGordon>,
    pub prince: Option<Prince<hal::Enabled>>,
    pub rng: Option<hal::peripherals::rng::Rng<hal::Enabled>>,
}

/// Initialized filesystem.
pub struct Filesystem {
    pub store: types::Store,
    pub internal_storage_fs: &'static mut Option<types::FlashStorage>,
}

/// Initialized everything that is needed, minus unecessary intermediates
pub struct All
{
    pub trussed: types::Trussed,
    pub filesystem: Filesystem,
    pub usb: Usb,
    pub interfaces: Interfaces,
    pub nfc: Nfc,
    pub basic: Basic,
    pub clock: Clock,
}


