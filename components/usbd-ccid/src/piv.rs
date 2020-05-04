use core::convert::TryFrom;

use cortex_m_semihosting::hprintln;
use heapless_bytes::consts;

use crate::{
    der::Der,
    types::{
        apdu,
    },
    constants::*,
    types::{
        MessageBuffer,
    },
};

fn write_apdu(sw1: u8, sw2: u8, data: &[u8], buffer: &mut MessageBuffer) {
    let l = data.len();
    buffer.clear();

    buffer.extend_from_slice(data).unwrap();
    buffer.push(sw1).unwrap();
    buffer.push(sw2).unwrap();
}

// top nibble of first byte is "category", here "A" = International
// this category has 5 byte "registered application provider identifier"
// (international RID, the other 9 nibbles is between 0x0 and 0x9).
pub const NIST_RID: [u8; 5] = [0xa0, 0x00, 0x00, 0x03, 0x08];
pub const NIST_PIX_PIV_APP: [u8; 4] = [0x00, 0x00, 0x10, 0x00];
pub const NIST_PIX_PIV_VERSION: [u8; 2] = [0x01, 0x00];
pub const PIV_PIX: [u8; 6] = [0x00, 0x00, 0x10, 0x00, 0x01, 0x00];
pub const PIV_AID: [u8; 11]
    = [0xa0, 0x00, 0x00, 0x03, 0x08, 0x00, 0x00, 0x10, 0x00, 0x01, 0x00];
pub const PIV_TRUNCATED_AID: [u8; 9]
    = [0xa0, 0x00, 0x00, 0x03, 0x08, 0x00, 0x00, 0x10, 0x00];

pub const SELECT: (u8, u8, u8, u8, usize) = (
    0x00, // interindustry, channel 0, no chain, no secure messaging,
    0xa4, // SELECT
    // p1
    0x04, // data is DF name, may be AID, possibly right-truncated
    // p2: i think this is dummy here
    0x00, // b2, b1 zero means "file occurence": first/only occurence,
          // b4, b3 zero means "file control information": return FCI template
    256,
);

//
// See SP 800-73 Part 1, Table 7
// for list of all objects and minimum container capacity
// - CCC: 287
// - CHUID: 2916
// - discovery: 19
// - key history: 256
// - x5c: 1905B
// - etc.
//
pub const GET_DATA: (u8, u8, u8, u8, usize) = (
    0x00, // as before, would be 0x0C for secure messaging
    0xCB, // GET DATA. There's also `CA`, setting bit 1 here
          // means (7816-4, sec. 5.1.2): use BER-TLV, as opposed
          // to "no indication provided".
    // P1, P2: 7816-4, sec. 7.4.1: bit 1 of INS set => P1,P2 identifies
    // a file. And 0x3FFF identifies current DF
    0x3F,
    0xFF,
    256,
);

// SW (SP 800-73 Part 1, Table 6)
// == == == == == == == == == == ==
// 61, xx success, more response data bytes
//
// 63, 00 verification failed
// 63, Cx verification failed, x furtehr retries or resets
//
// 68, 82 secure messaging not supported
//
// 69, 82 security status not satisfied
// 69, 83 authn method blocked
// :      (more secure messaging stuff)
//
// 6A, 80 incorrect parameter in command data field
// 6A, 81 function not supported
// 6A, 82 data object not found ( = NOT FOUND for files, e.g. certificate, e.g. after GET-DATA)
// 6A, 84 not enough memory
// 6A, 86 incorrect parameter in P1/P2
// 6A, 88 reference(d) data not found ( = NOT FOUND for keys, e.g. global PIN, e.g. after VERIFY)
//
// 90, 00 SUCCESS!
// == == == == == == == == == == ==

pub fn fake_piv(command: &mut MessageBuffer) {
    let apdu = match apdu::Apdu::try_from(command.as_mut()) {
        Ok(apdu) => apdu,
        Err(_) => {
            invalid_apdu(command);
            return;
        }
    };
    hprintln!(":: {:?}", &apdu).ok();

    let (cla, ins, p1, p2, le) = (*&apdu.cla(), apdu.ins(), apdu.p1(), apdu.p2(), apdu.le());

    match (cla, ins, p1, p2, le) {
        // `piv-tool -n` sends SELECT for 'A0 00 00 00 01 01', with Le = 0 (?!)
        // we need to handle this one
        SELECT => {
            hprintln!("got SELECT").ok();
            let is_piv = apdu.data() == &PIV_AID;
            let is_trunc_piv = apdu.data() == &PIV_TRUNCATED_AID;
            if is_piv || is_trunc_piv {
                hprintln!("for PIV").ok();
                select(command);
            }
        }
        _ => {
            panic!("unhandled APDU (0x{:x}, 0x{:x}, 0x{:x}, 0x{:x}, {}), !",
                cla, ins, p1, p2, le);
        }
    }
}

fn invalid_apdu(command: &mut MessageBuffer) {
    command.clear();
    // figure out what the correct error status words are
    command.extend_from_slice(&[0x6a, 0x82]);

}

fn select(command: &mut MessageBuffer) {
    let mut der: Der<consts::U256> = Default::default();
    der.nested(0x61, |der| {
        // Application identifier of application:
        // -> PIX (without RID, with version)
        der.raw_tlv(0x4f, &PIV_PIX)?;

        // Coexistent tag allocation authority
        der.nested(0x79, |der| {
            // Application identifier
            der.raw_tlv(0x4f, &NIST_RID)
        })?;

        // Application label (optional)
        // der.raw_tlv(0x50, ...);

        // URL to spec of app (optional)
        // der.raw_tlv(0x5f50, ...);

        // Cryptographic algorithms supported
        // Conditionally mandatory if only a suubset of SP 800-78
        // algorithms are supported.
        //
        // We do intend to leave out RSA!
        der.nested(0xac, |der| {
            // one entry per algorithm
            // der.raw_tlv(0x80, ...[SP800-78, Table 6-2]

            // 0x07: RSA2048
            der.raw_tlv(0x80, &[0x07])?;

            // // 0x08: AES128-ECB
            // der.raw_tlv(0x80, &[0x08])?;

            // // 0x0A: AES192-ECB
            // der.raw_tlv(0x80, &[0x0a])?;

            // 0x0C: AES256-ECB
            der.raw_tlv(0x80, &[0x0c])?;

            // 0x11: P256
            der.raw_tlv(0x80, &[0x11])?;

            // // 0x14: P384
            // der.raw_tlv(0x80, &[0x10])?;

            // 25519 (this is not part of the spec! idea is to
            // use `0xC0...0xCF` to map "custom" algorithms, behind
            // a Cargo feature)
            // der.raw_tlv(0x80, &[0xc0])?;

            // // 0x27: Cipher Suite 2 (secure messaging w/P256)
            // der.raw_tlv(0x80, &[0x27])?;

            // // 0x2E: Cipher Suite 7 (secure messaging w/P384)
            // der.raw_tlv(0x80, &[0x2e])?;

            // object identifier ("its value is set to 0x00")
            der.raw_tlv(0x07, &[0x00])
        })
    }).unwrap();

    command.clear();
    command.extend_from_slice(&der).unwrap();

    // // application not found
    // command.extend_from_slice(&[0x6a, 0x80]).unwrap();
    // successful exectuion
    command.extend_from_slice(&[0x90, 0x00]).unwrap();
}
