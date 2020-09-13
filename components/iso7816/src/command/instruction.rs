#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Instruction {
    Select,
    GetData,
    Verify,
    ChangeReferenceData,
    ResetRetryCounter,
    GeneralAuthenticate,
    PutData,
    GenerateAsymmetricKeyPair,
    GetResponse,
    ReadBinary,
    WriteBinary,
    Unknown(u8),
}

pub struct UnknownInstruction {}

impl core::convert::From<u8> for Instruction {
    fn from(ins: u8) -> Self {
        match ins {
            0x20 => Instruction::Verify,
            0x24 => Instruction::ChangeReferenceData,
            0x2c => Instruction::ResetRetryCounter,
            0x47 => Instruction::GenerateAsymmetricKeyPair,
            0x87 => Instruction::GeneralAuthenticate,
            0xa4 => Instruction::Select,
            0xc0 => Instruction::GetResponse,
            0xcb => Instruction::GetData,
            0xdb => Instruction::PutData,
            0xb0 => Instruction::ReadBinary,
            0xd0 => Instruction::WriteBinary,
            ins => Instruction::Unknown(ins),
        }
    }
}

impl core::convert::Into<u8> for Instruction {
    fn into(self) -> u8 {
        match self {
            Instruction::Verify => 0x20,
            Instruction::ChangeReferenceData => 0x24,
            Instruction::ResetRetryCounter => 0x2c,
            Instruction::GenerateAsymmetricKeyPair => 0x47,
            Instruction::GeneralAuthenticate => 0x87,
            Instruction::Select => 0xa4,
            Instruction::GetResponse => 0xc0,
            Instruction::GetData => 0xcb,
            Instruction::PutData => 0xdb,
            Instruction::ReadBinary => 0xb0,
            Instruction::WriteBinary => 0xd0,
            Instruction::Unknown(ins) => ins,
        }
    }
}

// impl core::convert::TryFrom<u8> for Instruction {
//     type Error = UnknownInstruction;

//     fn try_from(ins: u8) -> Result<Self, Self::Error> {
//         let instruction = match ins {
//             0x20 => Instruction::Verify,
//             0x24 => Instruction::ChangeReferenceData,
//             0x2c => Instruction::ResetRetryCounter,
//             0x47 => Instruction::GenerateAsymmetricKeyPair,
//             0x87 => Instruction::GeneralAuthenticate,
//             0xa4 => Instruction::Select,
//             0xc0 => Instruction::GetResponse,
//             0xcb => Instruction::GetData,
//             0xdb => Instruction::PutData,
//             _ => return Instruction::Unknown(ins),
//             Err(UnknownInstruction {})
//         };

//         Ok(instruction)
//     }
// }