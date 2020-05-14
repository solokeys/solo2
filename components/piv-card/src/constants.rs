// https://developers.yubico.com/PIV/Introduction/Yubico_extensions.html

pub const RID_LENGTH: usize = 5;

// top nibble of first byte is "category", here "A" = International
// this category has 5 byte "registered application provider identifier"
// (international RID, the other 9 nibbles is between 0x0 and 0x9).
pub const NIST_RID: &[u8; 5] = &[0xa0, 0x00, 0x00, 0x03, 0x08];
pub const YUBICO_RID: &[u8; 5] = &[0xa0, 0x00, 0x00, 0x05, 0x27];
// temp, until our application is through
pub const SOLOKEYS_RID: &[u8; 5] = &[0xa0, 0x00, 0x06, 0x06, 0x06];

pub const PIV_APP: [u8; 4] = [0x00, 0x00, 0x10, 0x00];
pub const PIV_VERSION: [u8; 2] = [0x01, 0x00];
pub const PIV_PIX: [u8; 6] = [0x00, 0x00, 0x10, 0x00, 0x01, 0x00];

pub const PIV_TRUNCATED_AID: [u8; 9]
    = [0xa0, 0x00, 0x00, 0x03, 0x08, 0x00, 0x00, 0x10, 0x00];

pub const PIV_AID: [u8; 11]
    = [0xa0, 0x00, 0x00, 0x03, 0x08, 0x00, 0x00, 0x10, 0x00, 0x01, 0x00];


// https://git.io/JfWuD
pub const YUBICO_OTP_PIX: &[u8; 3] = &[0x20, 0x01, 0x01];
pub const YUBICO_OTP_AID: [u8; 8] = [0xa0, 0x00, 0x00, 0x05, 0x27, 0x20, 0x01, 0x01];
// they use it to "deauthenticate user PIN and mgmt key": https://git.io/JfWgN
pub const YUBICO_MGMT_PIX: &[u8; 3] = &[0x47, 0x11, 0x17];
pub const YUBICO_MGMT_AID: &[u8; 8] = &[0xa0, 0x00, 0x00, 0x05, 0x27, 0x20, 0x01, 0x01];

// https://git.io/JfW28
// const (
// 	// https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-78-4.pdf#page=17
// 	algTag     = 0x80
// 	alg3DES    = 0x03
// 	algRSA1024 = 0x06
// 	algRSA2048 = 0x07
// 	algECCP256 = 0x11
// 	algECCP384 = 0x14

// 	// https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-78-4.pdf#page=16
// 	keyAuthentication     = 0x9a
// 	keyCardManagement     = 0x9b
// 	keySignature          = 0x9c
// 	keyKeyManagement      = 0x9d
// 	keyCardAuthentication = 0x9e
// 	keyAttestation        = 0xf9

// 	insVerify             = 0x20
// 	insChangeReference    = 0x24
// 	insResetRetry         = 0x2c
// 	insGenerateAsymmetric = 0x47
// 	insAuthenticate       = 0x87
// 	insGetData            = 0xcb
// 	insPutData            = 0xdb
// 	insSelectApplication  = 0xa4
// 	insGetResponseAPDU    = 0xc0

// 	// https://github.com/Yubico/yubico-piv-tool/blob/yubico-piv-tool-1.7.0/lib/ykpiv.h#L656
// 	insGetSerial     = 0xf8
// 	insAttest        = 0xf9
// 	insSetPINRetries = 0xfa
// 	insReset         = 0xfb
// 	insGetVersion    = 0xfd
// 	insImportKey     = 0xfe
// 	insSetMGMKey     = 0xff
// )

pub const OK: &[u8; 2] = &[0x90, 0x00];

// pub const SELECT: (u8, u8, u8, u8, usize) = (
pub const SELECT: (u8, u8, u8, u8) = (
    0x00, // interindustry, channel 0, no chain, no secure messaging,
    0xa4, // SELECT
    // p1
    0x04, // data is DF name, may be AID, possibly right-truncated
    // p2: i think this is dummy here
    0x00, // b2, b1 zero means "file occurence": first/only occurence,
          // b4, b3 zero means "file control information": return FCI template
    // 256,
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
// pub const GET_DATA: (u8, u8, u8, u8, usize) = (
pub const GET_DATA: (u8, u8, u8, u8) = (
    0x00, // as before, would be 0x0C for secure messaging
    0xCB, // GET DATA. There's also `CA`, setting bit 1 here
          // means (7816-4, sec. 5.1.2): use BER-TLV, as opposed
          // to "no indication provided".
    // P1, P2: 7816-4, sec. 7.4.1: bit 1 of INS set => P1,P2 identifies
    // a file. And 0x3FFF identifies current DF
    0x3F,
    0xFF,
    // 256,
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

//// ISO/IEC 7816-4, 5.1.3 "Status bytes"
//#[derive(Copy, Clone, Debug, Eq, PartialEq)]
//pub enum StatusWord {

////////////////////////////////
//// Normal processing (90, 61)
////////////////////////////////

//    // 9000
//    Success,

//    // 61XX
//    MoreAvailable(u8),

/////////////////////////////////
//// Warning processing (62, 63)
/////////////////////////////////

//    // 62XX: state of non-volatile memory unchanged (cf. SW2)

//    // 63XX: state of non-volatile memory changed (cf. SW2)
//    VerificationFailed,
//    FailedRetries(u8),

//////////////////////////////////
//// Execution error (64, 65, 66)
//////////////////////////////////

//    // 64XX: persistent memory unchanged (cf. SW2)
//    // 65XX: persistent memory changed (cf. SW2)
//    // 66XX: security related issues

/////////////////////////////////
//// Checking error (67 - 6F)
/////////////////////////////////

//    // 6700: wrong length, no further indication

//    // 68XX: functions in CLA not supported (cf. SW2)
//    SecureMessagingNotSupported,
//    CommandChainingNotSupported,

//    // 69xx: command not allowed (cf. SW2)
//    SecurityStatusNotSatisfied,
//    OperationBlocked,

//    // 6Axx: wrong parameters P1-P2 (cf. SW2)
//    IncorrectDataParameter,
//    FunctionNotSupported,
//    NotFound,
//    NotEnoughMemory,
//    IncorrectP1OrP2Parameter,
//    KeyReferenceNotFound,

//    // 6BXX: wrong parameters P1-P2

//    // 6CXX: wrong Le field, SW2 encodes available bytes

//    // 6D00: instruction code not supported or invalid
//    InstructionNotSupportedOrInvalid,

//    // 6E00: class not supported
//    ClassNotSupported,

//    // 6F00: no precise diagnosis
//    UnspecifiedCheckingError,
//}

//impl Into<u16> for StatusWord {
//    #[inline]
//    fn into(self) -> u16 {
//        match self {
//            Self::VerificationFailed => 0x6300,
//            Self::FailedRetries(x) => {
//                assert!(x < 16);
//                u16::from_be_bytes([0x63, 0xc0 + x])
//            }

//            Self::SecureMessagingNotSupported => 0x6882,
//            Self::CommandChainingNotSupported => 0x6884,

//            Self::SecurityStatusNotSatisfied => 0x6982,
//            Self::OperationBlocked => 0x6983,

//            Self::IncorrectDataParameter => 0x6a80,
//            Self::FunctionNotSupported => 0x6a81,
//            Self::NotFound => 0x6a82,
//            Self::NotEnoughMemory => 0x6a84,
//            Self::IncorrectP1OrP2Parameter => 0x6a86,
//            Self::KeyReferenceNotFound => 0x6a88,

//            Self::InstructionNotSupportedOrInvalid => 0x6d00,
//            Self::ClassNotSupported => 0x6e00,
//            Self::UnspecifiedCheckingError => 0x6f00,

//            Self::Success => 0x9000,
//            Self::MoreAvailable(x) => u16::from_be_bytes([0x61, x]),
//        }
//    }
//}

//impl Into<[u8; 2]> for StatusWord {
//    #[inline]
//    fn into(self) -> [u8; 2] {
//        let sw: u16 = self.into();
//        sw.to_be_bytes()
//    }
//}


// 6A, 80 incorrect parameter in command data field
// 6A, 81 function not supported
// 6A, 82 data object not found ( = NOT FOUND for files, e.g. certificate, e.g. after GET-DATA)
// 6A, 84 not enough memory
// 6A, 86 incorrect parameter in P1/P2
// 6A, 88 reference(d) data not found ( = NOT FOUND for keys, e.g. global PIN, e.g. after VERIFY)
//
// 90, 00 SUCCESS!
// == == == == == == == == == == ==

