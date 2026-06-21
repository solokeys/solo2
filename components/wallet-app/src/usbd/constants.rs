//! Constants for Wallet HID USB class

/// Packet size for Wallet HID (64 bytes)
pub const PACKET_SIZE: usize = 64;

/// Interrupt endpoint poll interval (5ms)
pub const INTERRUPT_POLL_MILLISECONDS: u8 = 5;

/// Maximum inbound message size (a request, reassembled across packets).
/// Tracks the dispatch request buffer so the two never drift.
pub const MESSAGE_SIZE: usize = crate::dispatch::types::REQUEST_SIZE;

/// Wallet HID report descriptor — vendor usage page 0xFFA0, the page host
/// wallet host tools match on, kept distinct from the FIDO usage page
/// (0xF1D0) so the two HID interfaces don't collide in a composite USB device.
pub const HID_REPORT_DESCRIPTOR_LENGTH: usize = 34;
pub const HID_REPORT_DESCRIPTOR: [u8; HID_REPORT_DESCRIPTOR_LENGTH] = [
    // Usage page (vendor defined): 0xFFA0
    0x06,
    0xA0,
    0xFF,
    // Usage ID (vendor defined): 0x1 (wallet)
    0x09,
    0x01,
    // Collection (application)
    0xA1,
    0x01,
    // The Input report
    0x09,
    0x20, // Usage ID - vendor defined: wallet input
    0x15,
    0x00, // Logical Minimum (0)
    0x26,
    0xFF,
    0x00, // Logical Maximum (255)
    0x75,
    0x08, // Report Size (8 bits)
    0x95,
    PACKET_SIZE as u8, // Report Count (64 fields)
    0x81,
    0x02, // Input (Data, Variable, Absolute)
    // The Output report
    0x09,
    0x21, // Usage ID - vendor defined: wallet output
    0x15,
    0x00, // Logical Minimum (0)
    0x26,
    0xFF,
    0x00, // Logical Maximum (255)
    0x75,
    0x08, // Report Size (8 bits)
    0x95,
    PACKET_SIZE as u8, // Report Count (64 fields)
    0x91,
    0x02, // Output (Data, Variable, Absolute)
    // EndCollection
    0xC0,
];

pub const HID_INTERFACE_CLASS: u8 = 0x03;
pub const INTERFACE_SUBCLASS_NONE: u8 = 0x0;
pub const INTERFACE_PROTOCOL_NONE: u8 = 0x0;

pub const HID_DESCRIPTOR: u8 = 0x21;
pub const HID_REPORT_DESCRIPTOR_TYPE: u8 = 0x22;

/// Ledger HID frame header: [channel(2), tag(1), seq(2), ...].
/// The channel is host-chosen and echoed back (see `WalletHid::channel`); the
/// tag identifies the packet type — `0x05` = APDU.
pub const APDU_TAG: u8 = 0x05;
/// Channel id used before a request sets one (and by our own CLI).
pub const DEFAULT_CHANNEL: [u8; 2] = [0x01, 0x01];
