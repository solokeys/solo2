pub const PACKET_SIZE: usize = 64;

// 7609 bytes
pub const MESSAGE_SIZE: usize = PACKET_SIZE - 7 + 128 * (PACKET_SIZE - 5);

