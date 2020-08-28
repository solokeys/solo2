use hid_dispatch::app::{App, Command, Message, Response};
use hid_dispatch::command::VendorCommand;

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

const REBOOT_COMMAND: hid_dispatch::command::Command = Command::Vendor(VendorCommand::H53);

impl App for Wink {
    fn commands(&self) -> &'static [Command] {
        &[Command::Wink, REBOOT_COMMAND]
    }

    fn call(&mut self, command: Command, message: &mut Message) -> Response {
        match command {
            Command::Vendor(VendorCommand::H53) => {
                // REBOOT
                crate::hal::raw::SCB::sys_reset();
            }
            _ => {
                self.got_wink = true;
                message.clear();
            }
        }
        Ok(())
    }
}