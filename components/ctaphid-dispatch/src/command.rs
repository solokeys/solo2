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

    // ISO7816 only commands
    Deselect,

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
            0x12 => Ok(Command::Deselect),
            0x3b => Ok(Command::KeepAlive),
            code => Ok(Command::Vendor(VendorCommand::try_from(code)?)),
        }
    }
}

/// Vendor CTAPHID commands, from 0x40 to 0x7f.
#[repr(u8)]
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub enum VendorCommand {
    H40 = 0x40,
    H41 = 0x41,
    H42 = 0x42,
    H43 = 0x43,
    H44 = 0x44,
    H45 = 0x45,
    H46 = 0x46,
    H47 = 0x47,
    H48 = 0x48,
    H49 = 0x49,
    H4A = 0x4A,
    H4B = 0x4B,
    H4C = 0x4C,
    H4D = 0x4D,
    H4E = 0x4E,
    H4F = 0x4F,
    H50 = 0x50,
    H51 = 0x51,
    H52 = 0x52,
    H53 = 0x53,
    H54 = 0x54,
    H55 = 0x55,
    H56 = 0x56,
    H57 = 0x57,
    H58 = 0x58,
    H59 = 0x59,
    H5A = 0x5A,
    H5B = 0x5B,
    H5C = 0x5C,
    H5D = 0x5D,
    H5E = 0x5E,
    H5F = 0x5F,
    H60 = 0x60,
    H61 = 0x61,
    H62 = 0x62,
    H63 = 0x63,
    H64 = 0x64,
    H65 = 0x65,
    H66 = 0x66,
    H67 = 0x67,
    H68 = 0x68,
    H69 = 0x69,
    H6A = 0x6A,
    H6B = 0x6B,
    H6C = 0x6C,
    H6D = 0x6D,
    H6E = 0x6E,
    H6F = 0x6F,
    H70 = 0x70,
    H71 = 0x71,
    H72 = 0x72,
    H73 = 0x73,
    H74 = 0x74,
    H75 = 0x75,
    H76 = 0x76,
    H77 = 0x77,
    H78 = 0x78,
    H79 = 0x79,
    H7A = 0x7A,
    H7B = 0x7B,
    H7C = 0x7C,
    H7D = 0x7D,
    H7E = 0x7E,
    H7F = 0x7F,
}

impl VendorCommand {
    pub const FIRST: u8 = 0x40;
    pub const LAST: u8 = 0x7f;
}


impl TryFrom<u8> for VendorCommand {
    type Error = ();

    fn try_from(from: u8) -> core::result::Result<Self, ()> {
        match from {
            // code if code >= Self::FIRST && code <= Self::LAST => Ok(VendorCommand(code)),
            code @ Self::FIRST..=Self::LAST => Ok(unsafe { core::mem::transmute(code) }),
            // TODO: replace with Command::Unknown and infallible Try
            _ => Err(()),
        }
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
            Command::Deselect => 0x12,
            Command::KeepAlive => 0x3b,
            Command::Vendor(command) => command as u8,
        }
    }
}

