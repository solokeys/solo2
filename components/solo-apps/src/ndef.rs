//! NDEF-suppression wrappers shared by both runners.
//!
//! The NDEF app refuses `SELECT` (so phones don't pop the tag during/after a
//! FIDO ceremony) while `now() - last_fido() < SUPPRESS_WINDOW`. The timebase
//! and the FIDO-transport hook are board-specific and supplied via `NfcClock`;
//! the wrappers themselves carry no per-instance state.

use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering::Relaxed};

/// Board-specific NDEF timebase + FIDO-transport hook.
pub trait NfcClock {
    /// Suppression window, same unit as `now()` (lpc55 7 s, nrf52840dk 3000 ms).
    const SUPPRESS_WINDOW: u32;
    /// Free-running counter, valid in passive (lpc55 RTC COUNT, nrf52840dk ms task).
    fn now() -> u32;
    /// Board-owned static stamped on every FIDO select/call. Initialised
    /// `now() - SUPPRESS_WINDOW` "in the past" so the tag is readable at boot.
    fn last_fido() -> &'static AtomicU32;
    /// Record whether the in-flight FIDO request arrived contactless, so
    /// `check_user_presence` takes the tap as presence. Default no-op.
    fn set_fido_over_nfc(_contactless: bool) {}
}

/// Wraps the FIDO app: stamps the suppression window on every FIDO select/call.
pub struct FidoNdefStamp<'a, C: NfcClock, A>(&'a mut A, PhantomData<C>);

impl<'a, C: NfcClock, A> FidoNdefStamp<'a, C, A> {
    pub fn new(app: &'a mut A) -> Self {
        Self(app, PhantomData)
    }
}

impl<C: NfcClock, A: apdu_dispatch::iso7816::App> apdu_dispatch::iso7816::App
    for FidoNdefStamp<'_, C, A>
{
    fn aid(&self) -> apdu_dispatch::iso7816::Aid {
        self.0.aid()
    }
}

impl<C: NfcClock, A: apdu_dispatch::app::App> apdu_dispatch::app::App for FidoNdefStamp<'_, C, A> {
    fn select(
        &mut self,
        interface: apdu_dispatch::app::Interface,
        command: apdu_dispatch::app::CommandView<'_>,
        reply: &mut apdu_dispatch::app::VecView<u8>,
    ) -> apdu_dispatch::app::Result {
        C::last_fido().store(C::now(), Relaxed);
        self.0.select(interface, command, reply)
    }
    fn deselect(&mut self) {
        self.0.deselect()
    }
    fn call(
        &mut self,
        interface: apdu_dispatch::app::Interface,
        command: apdu_dispatch::app::CommandView<'_>,
        reply: &mut apdu_dispatch::app::VecView<u8>,
    ) -> apdu_dispatch::app::Result {
        C::last_fido().store(C::now(), Relaxed);
        // Record the transport so check_user_presence takes the tap as presence
        // for an NFC (contactless) request and requires a button otherwise.
        C::set_fido_over_nfc(matches!(
            interface,
            apdu_dispatch::app::Interface::Contactless
        ));
        self.0.call(interface, command, reply)
    }
}

/// Wraps the NDEF app: refuses `SELECT` (so phones see no tag) while suppressed.
pub struct NdefFidoGate<'a, C: NfcClock, A>(&'a mut A, PhantomData<C>);

impl<'a, C: NfcClock, A> NdefFidoGate<'a, C, A> {
    pub fn new(app: &'a mut A) -> Self {
        Self(app, PhantomData)
    }
}

impl<C: NfcClock, A: apdu_dispatch::iso7816::App> apdu_dispatch::iso7816::App
    for NdefFidoGate<'_, C, A>
{
    fn aid(&self) -> apdu_dispatch::iso7816::Aid {
        self.0.aid()
    }
}

impl<C: NfcClock, A: apdu_dispatch::app::App> apdu_dispatch::app::App for NdefFidoGate<'_, C, A> {
    fn select(
        &mut self,
        interface: apdu_dispatch::app::Interface,
        command: apdu_dispatch::app::CommandView<'_>,
        reply: &mut apdu_dispatch::app::VecView<u8>,
    ) -> apdu_dispatch::app::Result {
        let since = C::now().wrapping_sub(C::last_fido().load(Relaxed));
        if since < C::SUPPRESS_WINDOW {
            return Err(apdu_dispatch::iso7816::Status::NotFound);
        }
        self.0.select(interface, command, reply)
    }
    fn deselect(&mut self) {
        self.0.deselect()
    }
    fn call(
        &mut self,
        interface: apdu_dispatch::app::Interface,
        command: apdu_dispatch::app::CommandView<'_>,
        reply: &mut apdu_dispatch::app::VecView<u8>,
    ) -> apdu_dispatch::app::Result {
        self.0.call(interface, command, reply)
    }
}
