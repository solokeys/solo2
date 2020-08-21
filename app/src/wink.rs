use hid_dispatch::app::{App, Command, Message, Response};

pub struct Wink{
    got_wink: bool,
}

impl Wink {
    pub fn new() -> Self {
        Wink {got_wink: false}
    }

    /// Indicate if a wink was recieved
    pub fn wink(&mut self) -> bool {
        if self.got_wink {
            self.got_wink = false;
            true
        } else {
            false
        }
    }
}

impl App for Wink {
    fn commands(&self) -> &'static [Command] {
        &[Command::Wink]
    }

    fn call(&mut self, _command: Command, message: &mut Message) -> Response {
        self.got_wink = true;
        message.clear();
        Ok(())
    }
}