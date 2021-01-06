
use interchange::Interchange;
use interchange::{Responder, State};
use crate::types::{Command, Message, HidInterchange, Error};
use crate::app::App;

pub struct Dispatch {
    responder: Responder<HidInterchange>,
}


impl Dispatch {
    pub fn new(
        responder: Responder<HidInterchange>,
    ) -> Dispatch {
        Dispatch {
            responder,
        }
    }

    // Using helper here to take potentially large stack burden off of call chain to application.
    #[inline(never)]
    fn reply_with_request_buffer(&mut self){
        let (_command, message) = self.responder.take_request().unwrap();
        self.responder.respond(Ok(message)).expect("responder failed");
    }

    // Using helper here to take potentially large stack burden off of call chain to application.
    #[inline(never)]
    fn reply_with_error(&mut self, error: Error){
        let (_command, _message) = self.responder.take_request().unwrap();
        self.responder.respond(
            Err(error)
        ).expect("cant respond");
    }

    fn find_app<'a, 'b>(
        command: Command,
        apps: &'a mut [&'b mut dyn App]
    ) -> Option<&'a mut &'b mut dyn App> {

        apps.iter_mut().find(|app|
            app.commands().contains(&command)
        )
    }



    pub fn poll<'a>(
        &mut self,
        apps: &'a mut [&'a mut dyn App],
    ) {
        if State::Requested == self.responder.state() {
            let tuple: &mut (Command, Message) = unsafe { self.responder.interchange.as_mut().unwrap().rq_mut() };
            let command = tuple.0;
            let message = &mut tuple.1;
            let _commandu8: u8 = command.into();
            info_now!("cmd: {}", _commandu8);
            if let Some(app) = Self::find_app(command, apps) {
                match app.call(command, message) {
                    Err(err) => {
                        info!("error from hid app!");
                        self.reply_with_error(err);
                    }
                    Ok(()) => {
                        self.reply_with_request_buffer();
                    }
                }
            } else {
                self.reply_with_error(Error::InvalidCommand);
            }
        }

    }

}
