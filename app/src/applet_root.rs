use core::convert::TryInto;
use hid_dispatch::app::{self as hid, App, Command as HidCommand, Message, Response};
use hid_dispatch::command::VendorCommand;
use apdu_dispatch::applet;
use apdu_dispatch::iso7816::{Command, Status};
use trussed::{
    syscall,
    Client as TrussedClient,
};

pub struct Root {
    got_wink: bool,
    trussed: TrussedClient,
}

impl Root {
    pub fn new(client: TrussedClient) -> Self {
        Self {got_wink: false, trussed: client}
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

    fn user_present(&mut self) -> bool {
        let user_present = syscall!(self.trussed.confirm_user_present(15_000)).result;
        user_present.is_ok()
    }

    fn boot_to_bootrom() -> ! {
        // Best way to boot into MCUBOOT is to erase the first flash page before rebooting.
        use crate::hal::traits::flash::WriteErase;
        let flash = unsafe { crate::hal::peripherals::flash::Flash::steal() }.enabled(
            &mut unsafe {crate::hal::peripherals::syscon::Syscon::steal()}
        );
        crate::hal::drivers::flash::FlashGordon::new(flash).erase_page(0).ok();
        crate::hal::raw::SCB::sys_reset()
    }
}

const UPDATE_COMMAND: hid_dispatch::command::Command = HidCommand::Vendor(VendorCommand::H51);
const REBOOT_COMMAND: hid_dispatch::command::Command = HidCommand::Vendor(VendorCommand::H53);
const RNG_COMMAND: hid_dispatch::command::Command = HidCommand::Vendor(VendorCommand::H60);
const VERSION_COMMAND: hid_dispatch::command::Command = HidCommand::Vendor(VendorCommand::H61);
// const UUID_COMMAND: hid_dispatch::command::Command = HidCommand::Vendor(VendorCommand::H62);

impl App for Root {
    fn commands(&self) -> &'static [HidCommand] {
        &[HidCommand::Wink, REBOOT_COMMAND, UPDATE_COMMAND, RNG_COMMAND, VERSION_COMMAND]
    }

    fn call(&mut self, command: HidCommand, message: &mut Message) -> Response {
        match command {
            HidCommand::Vendor(VendorCommand::H53) => {
                // REBOOT
                crate::hal::raw::SCB::sys_reset();
            }
            HidCommand::Vendor(VendorCommand::H51) => {
                // BOOT TO UPDATE MODE
                if self.user_present() {
                    Self::boot_to_bootrom();
                } else {
                    return Err(hid::Error::InvalidLength);
                }
            }
            HidCommand::Vendor(VendorCommand::H60) => {
                // GET RNG
                // Fill the HID packet (57 bytes)
                message.clear();
                message.extend_from_slice(
                    &syscall!(self.trussed.random_bytes(57)).bytes.as_slice()
                ).ok();
            }
            HidCommand::Vendor(VendorCommand::H61) => {
                // GET VERSION
                message.clear();
                message.push(5).ok();
                message.push(0).ok();
                message.push(0).ok();
                message.push(0).ok();
            }
            _ => {
                message.clear();
                self.got_wink = true;
            }
        }
        Ok(())
    }
}

impl applet::Aid for Root {
    // Solo root app
    fn aid(&self) -> &'static [u8] {
        &[ 0xA0, 0x00, 0x00, 0x08, 0x47, 0x00, 0x00, 0x00, 0x01]
    }
    fn right_truncated_length(&self) -> usize {
        9
    }
}

impl applet::Applet for Root {


    fn select(&mut self, _apdu: &Command) -> applet::Result {
        Ok(Default::default())
    }

    fn deselect(&mut self) {}

    fn call(&mut self, interface_type: applet::InterfaceType, apdu: &Command) -> applet::Result {
        let instruction: u8 = apdu.instruction().into();

        match instruction {
            0x53 => {
                // Reboot
                crate::hal::raw::SCB::sys_reset();
            }
            0x51 => {
                // Boot to mcuboot (only when contact interface)
                if interface_type == applet::InterfaceType::Contact && self.user_present() {
                    // Doesn't return.
                    Self::boot_to_bootrom();
                }
                Err(Status::ConditionsOfUseNotSatisfied)
            }

            0x60 => {
                // Random bytes
                Ok(applet::Response::Respond(
                    syscall!(self.trussed.random_bytes(57)).bytes.as_slice().try_into().unwrap()
                ))
            }
            0x61 => {
                // Get version
                Ok(applet::Response::Respond(
                    (&[0x05u8, 0,0,0][..]).try_into().unwrap()
                ))
            }

            _ => {
                Err(Status::InstructionNotSupportedOrInvalid)
            }
        }

    }
} 
