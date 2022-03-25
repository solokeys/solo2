use crate::types::*;

pub fn poll_dispatchers(apdu_dispatch: &mut ApduDispatch,
			ctaphid_dispatch: &mut CtaphidDispatch,
			apps: &mut Apps) -> (bool, bool) {
	let apdu_poll = apps.apdu_dispatch(|apps| apdu_dispatch.poll(apps));
	let ctaphid_poll = apps.ctaphid_dispatch(|apps| ctaphid_dispatch.poll(apps));

	( apdu_poll == Some(apdu_dispatch::dispatch::Interface::Contact) || ctaphid_poll,
		apdu_poll == Some(apdu_dispatch::dispatch::Interface::Contactless) )
}

pub fn poll_usb_classes(usb_classes_opt: &mut Option<usbnfc::UsbClasses>) -> (bool, bool) {
	if usb_classes_opt.is_none() {
		return (false, false);
	}

	let usb_classes = usb_classes_opt.as_mut().unwrap();

	usb_classes.ctaphid.check_timeout(0); //TODO
	usb_classes.poll();
	(false, false)
}

pub fn run_trussed(trussed: &mut Trussed) {
	trussed.process();
}
