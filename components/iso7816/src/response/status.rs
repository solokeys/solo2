impl Default for Status {
    fn default() -> Self {
        Status::Success
    }
}

// I0x6985SO/IEC 7816-4, 5.1.3 "Status bytes"
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Status {

//////////////////////////////
// Normal processing (90, 61)
//////////////////////////////

    /// 9000
    Success,

    /// 61XX
    MoreAvailable(u8),

///////////////////////////////
// Warning processing (62, 63)
///////////////////////////////

    // 62XX: state of non-volatile memory unchanged (cf. SW2)

    // 63XX: state of non-volatile memory changed (cf. SW2)
    VerificationFailed,
    RemainingRetries(u8),

////////////////////////////////
// Execution error (64, 65, 66)
////////////////////////////////

    // 64XX: persistent memory unchanged (cf. SW2)
    UnspecifiedNonpersistentExecutionError,

    // 65XX: persistent memory changed (cf. SW2)
    UnspecifiedPersistentExecutionError,

    // 66XX: security related issues

///////////////////////////////
// Checking error (67 - 6F)
///////////////////////////////

    // 6700: wrong length, no further indication
    WrongLength,

    // 68XX: functions in CLA not supported (cf. SW2)
    LogicalChannelNotSupported,
    SecureMessagingNotSupported,
    CommandChainingNotSupported,

    // 69xx: command not allowed (cf. SW2)
    SecurityStatusNotSatisfied,
    ConditionsOfUseNotSatisfied,
    OperationBlocked,

    // 6Axx: wrong parameters P1-P2 (cf. SW2)
    IncorrectDataParameter,
    FunctionNotSupported,
    NotFound,
    NotEnoughMemory,
    IncorrectP1OrP2Parameter,
    KeyReferenceNotFound,

    // 6BXX: wrong parameters P1-P2

    // 6CXX: wrong Le field, SW2 encodes available bytes

    // 6D00: instruction code not supported or invalid
    InstructionNotSupportedOrInvalid,

    // 6E00: class not supported
    ClassNotSupported,

    // 6F00: no precise diagnosis
    UnspecifiedCheckingError,
}

impl Into<u16> for Status {
    #[inline]
    fn into(self) -> u16 {
        match self {
            Self::VerificationFailed => 0x6300,
            Self::RemainingRetries(x) => {
                assert!(x < 16);
                u16::from_be_bytes([0x63, 0xc0 + x])
            }

            Self::UnspecifiedNonpersistentExecutionError => 0x6400,
            Self::UnspecifiedPersistentExecutionError => 0x6500,

            Self::WrongLength => 0x6700,

            Self::LogicalChannelNotSupported => 0x6881,
            Self::SecureMessagingNotSupported => 0x6882,
            Self::CommandChainingNotSupported => 0x6884,

            Self::SecurityStatusNotSatisfied => 0x6982,
            Self::ConditionsOfUseNotSatisfied => 0x6985,
            Self::OperationBlocked => 0x6983,

            Self::IncorrectDataParameter => 0x6a80,
            Self::FunctionNotSupported => 0x6a81,
            Self::NotFound => 0x6a82,
            Self::NotEnoughMemory => 0x6a84,
            Self::IncorrectP1OrP2Parameter => 0x6a86,
            Self::KeyReferenceNotFound => 0x6a88,

            Self::InstructionNotSupportedOrInvalid => 0x6d00,
            Self::ClassNotSupported => 0x6e00,
            Self::UnspecifiedCheckingError => 0x6f00,

            Self::Success => 0x9000,
            Self::MoreAvailable(x) => u16::from_be_bytes([0x61, x]),
        }
    }
}

impl Into<[u8; 2]> for Status {
    #[inline]
    fn into(self) -> [u8; 2] {
        let sw: u16 = self.into();
        sw.to_be_bytes()
    }
}

