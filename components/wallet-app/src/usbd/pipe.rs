//! Transport protocol handling for Wallet HID

use crate::usbd::constants::APDU_TAG;
use iso7816::command::FromSliceError;

/// Unwrap transport protocol from raw bytes and parse as ISO7816 command
///
/// Format (for outdated_app=false):
/// - Transport header (5 bytes): [0x01, 0x01, 0x05, seq_high, seq_low]
/// - APDU payload header (7 bytes, first packet only): [total_len_high, total_len_low, CLA, INS, P1, P2, data_len]
///   where total_len = data_len + 5 (the 5 is for CLA + INS + P1 + P2 + data_len byte)
/// - Data payload
///
/// After skipping the transport header (5 bytes) and total_len (2 bytes), we have:
/// [CLA, INS, P1, P2, data_len, data...] which is exactly the ISO7816 format
pub fn unwrap_transport<const S: usize>(
    bytes: &[u8],
) -> Result<iso7816::Command<S>, FromSliceError> {
    const TRANSPORT_HEADER_LEN: usize = 5;
    const TOTAL_LEN_BYTES: usize = 2;
    const MIN_LENGTH: usize = TRANSPORT_HEADER_LEN + TOTAL_LEN_BYTES + 5; // transport + total_len + CLA+INS+P1+P2+data_len

    // Check minimum length
    if bytes.len() < MIN_LENGTH {
        return Err(FromSliceError::InvalidSliceLength);
    }

    // Header is [channel(2), tag(1), seq(2)]. The channel is host-chosen, so
    // only the APDU tag is checked (matching `WalletHid::read_and_handle_packet`).
    if bytes[2] != APDU_TAG {
        return Err(FromSliceError::InvalidSliceLength);
    }

    // Skip transport header (5 bytes) and total_len (2 bytes)
    // The remaining bytes are in ISO7816 format: [CLA, INS, P1, P2, data_len, data...]
    let apdu_start = TRANSPORT_HEADER_LEN + TOTAL_LEN_BYTES;

    // Read the data_len byte to determine the actual APDU length
    // APDU format: [CLA, INS, P1, P2, data_len, data...]
    // We need at least 5 bytes (CLA + INS + P1 + P2 + data_len)
    if bytes.len() < apdu_start + 5 {
        return Err(FromSliceError::InvalidSliceLength);
    }

    let data_len = bytes[apdu_start + 4] as usize;
    // Total APDU length: 4 bytes (CLA+INS+P1+P2) + 1 byte (data_len) + data_len bytes
    let apdu_total_len = 5 + data_len;

    // Make sure we have enough bytes
    if bytes.len() < apdu_start + apdu_total_len {
        return Err(FromSliceError::InvalidSliceLength);
    }

    // Extract only the APDU bytes (not the rest of the buffer)
    let apdu_bytes = &bytes[apdu_start..apdu_start + apdu_total_len];

    // Parse directly as ISO7816 command
    iso7816::Command::try_from(apdu_bytes)
}
