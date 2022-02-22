pub type UsbAccessType = u8; // note this is just a type definition, we depend on its size

/// Number of logical endpoints, including control
///
/// Despite the UM claiming that USB FS has 1 + 4 and USB HS has 1 + 5,
/// even the FS supports 1 + 5 endpoints.
pub const NUM_ENDPOINTS: usize = 1 + 5;
pub const BYTES_PER_EP_REGISTER: usize = 4*4;

pub const USB1_SRAM_ADDR: usize = 0x4010_0000;
pub const EP_MEM_ADDR: usize = USB1_SRAM_ADDR;
pub const EP_MEM_SIZE: usize = 0x4000;
pub const EP_REGISTERS_SIZE: usize = NUM_ENDPOINTS * BYTES_PER_EP_REGISTER;
