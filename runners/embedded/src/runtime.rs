use crate::types::*;
use crate::soc::types::Soc as SocT;
use crate::soc::trussed_ui::UserPresenceStatus;

pub fn poll_dispatchers(apdu_dispatch: &mut ApduDispatch,
			ctaphid_dispatch: &mut CtaphidDispatch,
			apps: &mut Apps) -> (bool, bool) {
	let apdu_poll = apps.apdu_dispatch(|apps| apdu_dispatch.poll(apps));
	let ctaphid_poll = apps.ctaphid_dispatch(|apps| ctaphid_dispatch.poll(apps));

	( apdu_poll == Some(apdu_dispatch::dispatch::Interface::Contact) || ctaphid_poll,
		apdu_poll == Some(apdu_dispatch::dispatch::Interface::Contactless) )
}

/* ************************************************************************ */

pub fn poll_usb<FA, FB, TA, TB, E>(usb_classes: &mut Option<usbnfc::UsbClasses>,
			ccid_spawner: FA, ctaphid_spawner: FB,
			t_now: embedded_time::duration::units::Milliseconds)
		where FA: Fn(<SocT as Soc>::Duration) -> Result<TA, E>,
			FB: Fn(<SocT as Soc>::Duration) -> Result<TB, E> {
	if usb_classes.is_none() {
		return;
	}

	let usb_classes = usb_classes.as_mut().unwrap();

	usb_classes.ctaphid.check_timeout(t_now.0);
	usb_classes.poll();

	maybe_spawn_ccid(usb_classes.ccid.did_start_processing(), ccid_spawner);
	maybe_spawn_ctaphid(usb_classes.ctaphid.did_start_processing(), ctaphid_spawner);
}

pub fn poll_nfc<F, T, E>(contactless: &mut Option<Iso14443>, nfc_spawner: F)
		where F: Fn(<SocT as Soc>::Duration) -> Result<T, E> {
	if contactless.is_none() {
		return;
	}

	let contactless = contactless.as_mut().unwrap();

	maybe_spawn_nfc(contactless.poll(), nfc_spawner);
}

/* ************************************************************************ */

pub fn ccid_keepalive<F, T, E>(usb_classes: &mut Option<usbnfc::UsbClasses>,
			ccid_spawner: F)
		where F: Fn(<SocT as Soc>::Duration) -> Result<T, E> {
	if usb_classes.is_none() {
		return;
	}

	let usb_classes = usb_classes.as_mut().unwrap();

	maybe_spawn_ccid(usb_classes.ccid.send_wait_extension(), ccid_spawner);
}

pub fn ctaphid_keepalive<F, T, E>(usb_classes: &mut Option<usbnfc::UsbClasses>,
			ctaphid_spawner: F)
		where F: Fn(<SocT as Soc>::Duration) -> Result<T, E> {
	if usb_classes.is_none() {
		return;
	}

	let usb_classes = usb_classes.as_mut().unwrap();

	// TODO: need UI "waiting-for-user-presence-P" flag
	let status = usb_classes.ctaphid.send_keepalive(
		crate::soc::trussed_ui::UserPresenceStatus::waiting()
	);
	match status {
		usbd_ctaphid::types::Status::ReceivedData(milliseconds) => {
			maybe_spawn_ctaphid(
				usb_classes.ctaphid.send_keepalive(true), ctaphid_spawner);
		}
		_ => {}
	}
	//maybe_spawn_ctaphid(usb_classes.ctaphid.send_keepalive(true), ctaphid_spawner);
}

pub fn nfc_keepalive<F, T, E>(contactless: &mut Option<Iso14443>, nfc_spawner: F)
		where F: Fn(<SocT as Soc>::Duration) -> Result<T, E> {
	if contactless.is_none() {
		return;
	}

	let contactless = contactless.as_mut().unwrap();

	maybe_spawn_nfc(contactless.poll_wait_extensions(), nfc_spawner);
}

/* ************************************************************************ */

fn maybe_spawn_ccid<F, T, E>(status: usbd_ccid::types::Status, ccid_spawner: F)
		where F: Fn(<SocT as Soc>::Duration) -> Result<T, E> {
	match status {
		usbd_ccid::types::Status::ReceivedData(ms) => { ccid_spawner(ms.into()).ok(); }
		_ => {}
	};
}

fn maybe_spawn_ctaphid<F, T, E>(status: usbd_ctaphid::types::Status, ctaphid_spawner: F)
		where F: Fn(<SocT as Soc>::Duration) -> Result<T, E> {
	match status {
		usbd_ctaphid::types::Status::ReceivedData(ms) => { ctaphid_spawner(ms.into()).ok(); }
		_ => {}
	};
}

fn maybe_spawn_nfc<F, T, E>(status: nfc_device::Iso14443Status, nfc_spawner: F)
		where F: Fn(<SocT as Soc>::Duration) -> Result<T, E> {
	match status {
		nfc_device::Iso14443Status::ReceivedData(ms) => { nfc_spawner(ms.into()).ok(); }
		_ => {}
	};
}

/* ************************************************************************ */

pub fn run_trussed(trussed: &mut Trussed) {
	trussed.process();
}
