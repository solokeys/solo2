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
use crate::types;
use hal::drivers::{clocks::Clocks, flash::FlashGordon, pins::direction, Timer};
use hal::peripherals::pfr::Pfr;
use hal::peripherals::prince::Prince;
use hal::typestates::pin::state::Gpio;

/// Initialized clocks, Nfc interrupt pin, Iocon, Gpio.
#[non_exhaustive]
pub struct Clock {
    pub clocks: Clocks,
    pub nfc_irq: Option<hal::Pin<board::nfc::NfcIrqPin, Gpio<direction::Input>>>,
    pub iocon: hal::Iocon<hal::Enabled>,
    pub gpio: hal::Gpio<hal::Enabled>,
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

/// Initialized NFC Iso14443 transport. The FM11NC08 and the external flash
/// share Spi0 via `board::shared_spi`; `iso14443` is `None` only when the NFC
/// chip is absent.
pub struct Nfc {
    pub iso14443: Option<nfc_device::Iso14443<'static, board::nfc::NfcChip>>,

    pub contactless_responder: Option<apdu_dispatch::interchanges::Responder<'static>>,
}

/// Initialized USB device + USB classes, Dynamic Clock controller.
pub struct Usb {
    pub usb_classes: Option<types::UsbClasses>,

    pub contact_responder: Option<apdu_dispatch::interchanges::Responder<'static>>,
    pub ctaphid_responder:
        Option<ctaphid_dispatch::Responder<'static, { ctaphid_dispatch::DEFAULT_MESSAGE_SIZE }>>,
    #[cfg(feature = "wallet")]
    pub wallet_responder: Option<types::WalletResponder>,
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
    pub internal_storage_fs: *mut types::FlashStorage,
}

/// Initialized everything that is needed, minus unecessary intermediates
pub struct All {
    pub trussed: types::Trussed,
    pub filesystem: Filesystem,
    pub usb: Usb,
    pub interfaces: Interfaces,
    pub nfc: Nfc,
    pub basic: Basic,
    pub clock: Clock,
}
