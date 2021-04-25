
use interchange::{Interchange, Responder};
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

    fn find_app<'a, 'b>(
        command: Command,
        apps: &'a mut [&'b mut dyn App]
    ) -> Option<&'a mut &'b mut dyn App> {

        apps.iter_mut().find(|app|
            app.commands().contains(&command)
        )
    }

    // // Using helper here to take potentially large stack burden off of call chain to application.
    // #[inline(never)]
    // fn reply_with_request_buffer(&mut self){
    //     let (_command, message) = self.responder.take_request().unwrap();
    //     let message = message.clone();
    //     self.responder.respond(&Ok(message)).expect("responder failed");
    // }

    // Using helper here to take potentially large stack burden off of call chain to application.
    #[inline(never)]
    fn reply_with_error(&mut self, error: Error){
        self.responder.respond(
            &Err(error)
        ).expect("cant respond");
    }

    #[inline(never)]
    fn call_app(&mut self, app: &mut dyn App, command: Command, request: &Message) {
        // now we do something that should be fixed conceptually later.
        // We will no longer use the interchange data as request (just cloned it)
        // We would like to pass the app a buffer to write data into - so we
        // use the "big enough" request reference for this (it would make much more
        // sense to use the response mut reference, but that's behind a Result).
        //
        // Note that this only works since Request has the same type as
        // Response's Ok value.
        let tuple: &mut (Command, Message) = unsafe { self.responder.interchange.rq_mut() };
        let response_buffer = &mut tuple.1;
        response_buffer.clear();

        if let Err(error) = app.call(command, &request, response_buffer) {
            self.reply_with_error(error)
        } else {
            let response = Ok(response_buffer.clone());
            self.responder.respond(&response).expect("responder failed");
        }
    }

    #[inline(never)]
    pub fn poll<'a>(
        &mut self,
        apps: &'a mut [&'a mut dyn App],
    ) -> bool {
        let maybe_request = self.responder.take_request();
        if let Some((command, message)) = maybe_request {
            info_now!("cmd: {}", u8::from(command));

            if let Some(app) = Self::find_app(command, apps) {
                // match app.call(command, self.responder.response_mut().unwrap()) {
                let request = message.clone();
                self.call_app(*app, command, &request);
            } else {
                self.reply_with_error(Error::InvalidCommand);
            }
        }

        self.responder.state() == interchange::State::Responded
    }

}
