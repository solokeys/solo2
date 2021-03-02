pub enum Error {
    VerificationFailed { remaining: u8 }, // 63 00 or 63 CX
    SecureMessagingNotSupported, // 68 82
    SecurityStatusNotSatisfied, // 69 82
    AuthenticationMethodBlocked,  // 69 83
    // ExpectedSecureMessagingDataObjectsMissing, // 69 87
    // SecureMessagingDataObjectsIncorrect, // 69 88
    IncorrectParameterInCommandDataField, // 6A 80
    FunctionNotSupported, // 6A 81
    DataObjectOrApplicationNotFound, // 6A 82
    NotEnoughMemory, // 6A 84
    IncorrecParameterInP1OrP2, // 6A 86
    ReferencedDataOrReferenceDataNotFound, // 6A 88
}

pub enum Success {
    Success,  // 61 xx
    SuccessResponseDataStillAvailable(u8), // 90 00
}

pub type Result = core::result::Result<Success, Error>;

// macro_rules! status_word {
//     ($($Name:ident: [$sw1:expr, $sw2:tt],)*) => {
//         $(
//             // pub struct $Name {}

//             status_word! ($Name, $sw1, $sw2);
//         )*

//         pub enum StatusWord {
//             $($Name($Name),)*
//         }
//     };

//     ($Name:ident, $sw1:expr, XX) => {
//         pub struct $Name {
//             sw2: u8,
//         }

//         impl $Name {
//             const SW1: u8 = $sw1;

//             pub fn new(sw2: u8) -> Self {
//                 Self { sw2 }
//             }

//             pub fn as_bytes(&self) -> [u8; 2] {
//                 [Self::SW1, self.sw2]
//             }

//         }

//         // impl core::ops::Deref for $Name {
//         //     type Target = [u8; 2];
//         //     fn deref(&self) -> &Self::Target {
//         //         &[Self::SW1, self.sw2]
//         //     }
//         // }

//     };

//     ($Name:ident, $sw1:expr, $sw2:expr) => {
//         #[derive(Default)]
//         pub struct $Name {}

//         impl $Name {
//             const SW1: u8 = $sw1;
//             const SW2: u8 = $sw2;

//             pub fn new() -> Self {
//                 Default::default()
//             }

//             pub fn as_bytes(&self) -> [u8; 2] {
//                 [Self::SW1, Self::SW2]
//             }
//         }
//     };
// }

// status_word! {
//     SecurityStatusNotSatisfied:  [0x69, 0x82],
//     NotFound:                    [0x6a, 0x82],
//     Success:                     [0x90, 0x00],

//     SuccessBytesRemaining:       [0x61,  XX ],
// }

// pub trait StatusWordTrait {
//     fn sw1(&self) -> u8;
//     fn sw2(&self) -> u8;
//
//     fn sw(&self) -> [u8; 2] {
//         [self.sw1(), self.sw2()]
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn deref() {
//         let sw = SuccessBytesRemaining::new(42);
//         println!("SW: {:?}", &sw.as_bytes());
//     }

// }

