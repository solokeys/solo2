use core::convert::TryInto;
use ctaphid_dispatch::app::{self as hid, Command as HidCommand, Message};
use ctaphid_dispatch::command::VendorCommand;
use apdu_dispatch::{Command, response, app as apdu};
use apdu_dispatch::{command::Size as CommandSize, response::Size as ResponseSize};
use apdu_dispatch::iso7816::Status;
use trussed::{
    syscall,
    Client as TrussedClient,
};

const UPDATE: VendorCommand = VendorCommand::H51;
const REBOOT: VendorCommand = VendorCommand::H53;
const RNG: VendorCommand = VendorCommand::H60;
const VERSION: VendorCommand = VendorCommand::H61;
const UUID: VendorCommand = VendorCommand::H62;

pub trait BootInterface {
    fn reboot() -> !;
    fn reboot_to_application_update() -> !;
}

pub struct App<T, BI>
where T: TrussedClient,
      BI: BootInterface,
{
    got_wink: bool,
    trussed: T,
    uuid: [u8; 16],
    version: u32,
    _boot_interface: Option<BI>,
}

impl<T, BI> App<T, BI>
where T: TrussedClient,
      BI: BootInterface,
{
    pub fn new(client: T, uuid: [u8; 16], version: u32) -> Self {
        Self {got_wink: false, trussed: client, uuid, version, _boot_interface: None}
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


}

impl<T,BI> hid::App for App<T,BI>
where T: TrussedClient,
      BI: BootInterface
{
    fn commands(&self) -> &'static [HidCommand] {
        &[
            HidCommand::Wink,
            HidCommand::Vendor(UPDATE),
            HidCommand::Vendor(REBOOT),
            HidCommand::Vendor(RNG),
            HidCommand::Vendor(VERSION),
            HidCommand::Vendor(UUID),
        ]
    }

    fn call(&mut self, command: HidCommand, _: &Message, response: &mut Message) -> hid::AppResult {
        match command {
            HidCommand::Vendor(REBOOT) => {
                BI::reboot();
            }
            HidCommand::Vendor(UPDATE) => {
                if self.user_present() {
                    BI::reboot_to_application_update();
                } else {
                    return Err(hid::Error::InvalidLength);
                }
            }
            HidCommand::Vendor(RNG) => {
                // Fill the HID packet (57 bytes)
                response.extend_from_slice(
                    &syscall!(self.trussed.random_bytes(57)).bytes.as_slice()
                ).ok();
            }
            HidCommand::Vendor(VERSION) => {
                // GET VERSION
                response.extend_from_slice(&self.version.to_be_bytes()).ok();
            }
            _ => {
                self.got_wink = true;
            }
        }
        Ok(())
    }
}

impl<T,BI> apdu::Aid for App<T,BI>
where T: TrussedClient,
      BI: BootInterface
{
    // Solo management app
    fn aid(&self) -> &'static [u8] {
        &[ 0xA0, 0x00, 0x00, 0x08, 0x47, 0x00, 0x00, 0x00, 0x01]
    }
    fn right_truncated_length(&self) -> usize {
        9
    }
}

impl<T,BI> apdu::App<CommandSize, ResponseSize> for App<T,BI>
where T: TrussedClient,
      BI: BootInterface
{

    fn select(&mut self, _apdu: &Command, _reply: &mut response::Data) -> apdu::Result {
        Ok(())
    }

    fn deselect(&mut self) {}

    fn call(&mut self, interface: apdu::Interface, apdu: &Command, reply: &mut response::Data) -> apdu::Result {
        let instruction: u8 = apdu.instruction().into();

        let command: VendorCommand = instruction.try_into().map_err(|_e| Status::InstructionNotSupportedOrInvalid)?;

        match command {
            REBOOT => {
                BI::reboot();
            }
            UPDATE => {
                // Boot to mcuboot (only when contact interface)
                if interface == apdu::Interface::Contact && self.user_present()
                {
                    BI::reboot_to_application_update();
                }
                return Err(Status::ConditionsOfUseNotSatisfied);
            }

            RNG => {
                // Random bytes
                reply.extend_from_slice(&syscall!(self.trussed.random_bytes(57)).bytes.as_slice()).ok();
            }
            VERSION => {
                // Get version
                reply.extend_from_slice(&self.version.to_be_bytes()[..]).ok();
            }

            UUID => {
                // Get UUID
                reply.extend_from_slice(&self.uuid).ok();
            }

            _ => {
                return Err(Status::InstructionNotSupportedOrInvalid);
            }

        }
        Ok(())

    }
}

