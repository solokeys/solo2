
// #[repr(u8)]
// #[derive(Copy, Clone, Debug)]
// pub enum MessageType {

//     // REQUESTS

//     // supported
//     PowerOn = 0x62,
//     PowerOff = 0x63,
//     GetSlotStatus = 0x65,
//     XfrBlock = 0x6f,
//     Abort = 0x72,

//     // unsupported
//     GetParameters = 0x6c,
//     ResetParameters = 0x6d,
//     SetParameters = 0x61,
//     Escape = 0x6b,//  for vendor commands
//     IccClock = 0x7e,
//     T0Apdu = 0x6a,
//     Secure = 0x69,
//     Mechanical = 0x71,
//     SetDataRateAndClockFrequency = 0x73,

//     // RESPONSES

//     // used
//     DataBlock = 0x80,
//     SlotStatus = 0x81,

//     // unused
//     Parameters = 0x82,
//     // Escape = 0x83,
//     DataRateAndClockFrequency = 0x84,
// }

// pub struct SlotStatus {
//     seq: u8
// }

// impl SlotStatus {
//     pub fn new(seq: u8) -> Self {
//         Self { seq }
//     }
// }

// impl From<SlotStatus> for Packet {
//     fn from(slot_status: SlotStatus) -> Self {
//         let mut buffer = [0u8; PACKET_SIZE];
//         buffer[0] = 0x81;
//         // buffer[1..5] = 0, no extra data
//         // buffer[5] = 0, only one slot
//         buffer[6] = slot_status.seq;
//         // buffer[7] = 0, status
//         // buffer[8] = 0, error
//         // buffer[9] = 0, chain parameter, this is complete
//         Packet { buffer }
//     }
// }

// pub struct DataBlock<'a> {
//     seq: u8,
//     data: &'a [u8],

// }

// impl<'a> DataBlock<'a> {
//     pub fn new(seq: u8, data: &'a [u8]) -> Self {
//         assert!(data.len() + 10 <= PACKET_SIZE);
//         Self { seq, data }
//     }
// }

// impl<'a> From<DataBlock<'a>> for Packet {
//     fn from(data_block: DataBlock) -> Self {
//         let mut buffer = [0u8; PACKET_SIZE];
//         let len = data_block.data.len();
//         buffer[0] = 0x80;
//         buffer[1..5].copy_from_slice(&len.to_le_bytes());
//         // buffer[5] = 0, only one slot
//         buffer[6] = data_block.seq;
//         // buffer[7] = 0, status
//         // buffer[8] = 0, error
//         // buffer[9] = 0, chain parameter, this is complete
//         buffer[10..][..len].copy_from_slice(data_block.data);
//         Packet { buffer }
//     }
// }

// impl core::convert::TryFrom<u8> for MessageType {
//     type Error = ();

//     fn try_from(message_type_byte: u8) -> core::result::Result<Self, ()> {
//         Ok(match message_type_byte {
//             0x62 => Self::PowerOn,
//             0x63 => Self::PowerOff,
//             0x65 => Self::GetSlotStatus,
//             0x6f => Self::XfrBlock,
//             0x71 => Self::Mechanical,
//             0x80 => Self::DataBlock,
//             0x81 => Self::SlotStatus,
//             _ => return Err(()),
//         })
//     }
// }

