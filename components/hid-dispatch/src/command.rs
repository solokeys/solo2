use core::convert::TryFrom;

#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub enum Command {
    // mandatory for CTAP1
    Ping,
    Msg,
    Init,
    Error,

    // optional
    Wink,
    Lock,

    // mandatory for CTAP2
    Cbor,
    Cancel,
    KeepAlive,

    // vendor-assigned range from 0x40 to 0x7f
    Vendor(VendorCommand),
}

impl Command {
    pub fn into_u8(self) -> u8 {
        self.into()
    }
}

impl TryFrom<u8> for Command {
    type Error = ();

    fn try_from(from: u8) -> core::result::Result<Command, ()> {
        match from {
            0x01 => Ok(Command::Ping),
            0x03 => Ok(Command::Msg),
            0x06 => Ok(Command::Init),
            0x3f => Ok(Command::Error),
            0x08 => Ok(Command::Wink),
            0x04 => Ok(Command::Lock),
            0x10 => Ok(Command::Cbor),
            0x11 => Ok(Command::Cancel),
            0x3b => Ok(Command::KeepAlive),
            code => Ok(Command::Vendor(VendorCommand::try_from(code)?)),
        }
    }
}

/// Vendor CTAPHID commands, from 0x40 to 0x7f.
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub struct VendorCommand(u8);

impl VendorCommand {
    pub const FIRST: u8 = 0x40;
    pub const LAST: u8 = 0x7f;
}


impl TryFrom<u8> for VendorCommand {
    type Error = ();

    fn try_from(from: u8) -> core::result::Result<Self, ()> {
        match from {
            // code if code >= Self::FIRST && code <= Self::LAST => Ok(VendorCommand(code)),
            code @ Self::FIRST..=Self::LAST => Ok(VendorCommand(code)),
            // TODO: replace with Command::Unknown and infallible Try
            _ => Err(()),
        }
    }
}

impl Into<u8> for VendorCommand {
    fn into(self) -> u8 {
        self.0
    }
}

impl Into<u8> for Command {
    fn into(self) -> u8 {
        match self {
            Command::Ping => 0x01,
            Command::Msg => 0x03,
            Command::Init => 0x06,
            Command::Error => 0x3f,
            Command::Wink => 0x08,
            Command::Lock => 0x04,
            Command::Cbor => 0x10,
            Command::Cancel => 0x11,
            Command::KeepAlive => 0x3b,
            Command::Vendor(command) => command.into(),
        }
    }
}

