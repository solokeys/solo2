#![cfg_attr(not(test), no_std)]

pub mod constants;
pub mod state;

use core::convert::TryFrom;

use cortex_m_semihosting::{dbg, hprintln};
use heapless_bytes::consts;
use interchange::Responder;
use iso7816::{
    Command, Instruction, Status,
    response::{
        Result as ResponseResult,
        Data as ResponseData,
    },
};
use trussed::Client as Trussed;
use usbd_ccid::{
    // constants::*,
    der::Der,
    types::ApduInterchange,
};

use constants::*;

pub struct App
{
    interchange: Responder<ApduInterchange>,
    state: state::State,
    trussed: Trussed,
    // trussed: RefCell<Trussed>,
}

impl App
{
    pub fn new(
        trussed: Trussed,
        interchange: Responder<ApduInterchange>,
    )
        -> Self
    {
        // seems like RefCell is not the right thing, we want something like `Rc` instead,
        // which can be cloned and injected into other parts of the App that use Trussed.
        // let trussed = RefCell::new(trussed);
        Self {
            interchange,
            // state: state::State::new(trussed.clone()),
            state: Default::default(),
            trussed,
        }
    }

    pub fn poll(&mut self) {
        if let Some(request) = self.interchange.take_request() {
            self.handle(&request);
        }
    }

    fn handle(&mut self, command: &Command) {

        let result = self.try_handle(command);

        self.interchange.respond(result.into()).expect("can respond");
    }

    fn try_handle(&mut self, command: &Command) -> ResponseResult {

        // TEMP
        dbg!(self.state.persistent(&mut self.trussed).timestamp(&mut self.trussed));

        // handle CLA
        // - command chaining not supported
        // - secure messaging not supported
        // - only channel zero supported
        // - ensure INS known to us

        let class = command.class();

        if !class.chain().last_or_only() {
            return Err(Status::CommandChainingNotSupported);
        }

        if !class.secure_messaging().none() {
            return Err(Status::SecureMessagingNotSupported);
        }

        if class.channel() != Some(0) {
            return Err(Status::LogicalChannelNotSupported);
        }

        hprintln!("CLA = {:?}", &command.class()).ok();
        hprintln!("INS = {:?}", &command.instruction()).ok();
        hprintln!("P1 = {:X}, P2 = {:X}", command.p1, command.p2).ok();
        hprintln!("extended = {:?}", command.extended).ok();

        // hprintln!("INS = {:?}" &command.instruction()).ok();
        match command.instruction() {
            Instruction::Select => self.select(command),
            Instruction::GetData => self.get_data(command),
            Instruction::Verify => self.verify(command),

            Instruction::Unknown(ins) => {

                // see if it's a Yubico thing
                if let Ok(instruction) = YubicoPivExtension::try_from(ins) {
                    self.yubico_piv_extension(command, instruction)
                } else {
                    Err(Status::FunctionNotSupported)
                }
            }

            _ => Err(Status::FunctionNotSupported),
        }
    }

    fn yubico_piv_extension(&mut self, command: &Command, instruction: YubicoPivExtension) -> ResponseResult {
        hprintln!("yubico extension: {:?}", &instruction).ok();
        match instruction {
            YubicoPivExtension::GetSerial => {
                // make up a 4-byte serial
                let data = ResponseData::try_from_slice(
                    &[0x00, 0x52, 0xf7, 0x43]).unwrap();
                Ok(data)
            }

            YubicoPivExtension::GetVersion => {
                // make up a version, be >= 5.0.0
                let data = ResponseData::try_from_slice(
                    &[0x06, 0x06, 0x06]).unwrap();
                Ok(data)
            }

            YubicoPivExtension::Attest => {
                if command.p2 != 0x00 {
                    return Err(Status::IncorrectP1OrP2Parameter);
                }

                let slot = command.p1;

                if slot == 0x9a {
                    let data = ResponseData::try_from_slice(YUBICO_ATTESTATION_CERTIFICATE_FOR_9A).unwrap();
                    return Ok(data);
                }

                Err(Status::FunctionNotSupported)
            }

            _ => Err(Status::FunctionNotSupported),
        }
    }

    fn verify(&mut self, command: &Command) -> ResponseResult {
        // we only implement our own PIN, not global Pin, not OCC data, not pairing code
        if command.p2 != 0x80 {
            return Err(Status::KeyReferenceNotFound);
        }

        let p1 = command.p1;
        if p1 != 0x00 && p1 != 0xFF {
            return Err(Status::IncorrectP1OrP2Parameter);
        }

        // all above failures shall not change security status or retry counter

        // 1) If p1 is FF, "log out" of PIN
        if p1 == 0xFF {
            if command.data().len() != 0 {
                return Err(Status::IncorrectDataParameter);
            } else {
                self.state.runtime.app_security_status.pin_verified = false;
                return Ok(Default::default());
            }
        }

        // 2) Get retries (or whether verification is even needed) by passing no data
        if p1 == 0x00 && command.data().len() == 0 {
            if self.state.runtime.app_security_status.pin_verified {
                return Ok(Default::default());
            } else {
                let retries = self.state.persistent(&mut self.trussed).remaining_pin_retries();
                return Err(Status::RemainingRetries(retries));
            }
        }

        // if malformed PIN is sent, no security implication
        if command.data().len() != 8 {
            return Err(Status::IncorrectDataParameter);
        }

        let sent_pin = match state::Pin::try_new(&command.data()) {
            Ok(pin) => pin,
            _ => return Err(Status::IncorrectDataParameter),
        };

        // 3) Verify le PIN!
        let remaining_retries = self.state.persistent(&mut self.trussed).remaining_pin_retries();
        if remaining_retries == 0 {
            return Err(Status::OperationBlocked);
        }

        if self.state.persistent(&mut self.trussed).verify_pin(&sent_pin) {
            self.state.persistent(&mut self.trussed).reset_consecutive_pin_mismatches(&mut self.trussed);
            self.state.runtime.app_security_status.pin_verified = true;
            Ok(Default::default())

        } else {
            let remaining = self.state.persistent(&mut self.trussed).increment_consecutive_pin_mismatches(&mut self.trussed);
            Err(Status::RemainingRetries(remaining))
        }
    }

    fn get_data(&mut self, command: &Command) -> ResponseResult {
        if command.p1 != 0x3f || command.p2 != 0xff {
            return Err(Status::IncorrectP1OrP2Parameter);
        }

        // TODO: adapt `derp` and use a proper DER parser

        let data = command.data();

        if data.len() < 3 {
            return Err(Status::IncorrectDataParameter);
        }

        let tag = data[0];
        if tag != 0x5c {
            return Err(Status::IncorrectDataParameter);
        }

        let len = data[1] as usize;
        let data = &data[2..];
        if data.len() != len {
            return Err(Status::IncorrectDataParameter);
        }

        if data.len() == 0 || data.len() > 3 {
            return Err(Status::IncorrectDataParameter);
        }

        // lookup what is asked for
        hprintln!("looking up {:?}", data).ok();

        // TODO: check security status, else return Status::SecurityStatusNotSatisfied

        // Table 3, Part 1, SP 800-73-4
        // https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-73-4.pdf#page=30
        match data {
            DataObjects::DiscoveryObject => todo!("discovery object"),
            DataObjects::BiometricInformationTemplate => todo!("biometric information template"),

            // '5FC1 05' (351B)
            DataObjects::X509CertificateForPivAuthentication => {
                let data = ResponseData::try_from_slice(YUBICO_PIV_AUTHENTICATION_CERTIFICATE).unwrap();
                Ok(data)
            }

            // '5F FF01' (754B)
            YubicoObjects::AttestationCertificate => {
                let data = ResponseData::try_from_slice(YUBICO_ATTESTATION_CERTIFICATE).unwrap();
                Ok(data)
            }

            _ => return Err(Status::NotFound),
        }
    }

    fn select(&mut self, command: &Command) -> ResponseResult {
        use state::Aid;

        if command.data().starts_with(state::PivAid::rid()) {
            hprintln!("got PIV!").ok();

            if command.p1 != 0x04 || command.p2 != 0x00 {
                return Err(Status::IncorrectP1OrP2Parameter);
            }

            let mut der: Der<consts::U256> = Default::default();
            der.nested(0x61, |der| {
                // Application identifier of application:
                // -> PIX (without RID, with version)
                der.raw_tlv(0x4f, &PIV_PIX)?;

                // Coexistent tag allocation authority
                der.nested(0x79, |der| {
                    // Application identifier
                    der.raw_tlv(0x4f, NIST_RID)
                // })?;
                })
            }).unwrap();


            let response_data: ResponseData = der.try_convert_into().unwrap();
            // hprintln!("reponse data: {:?}", &response_data).ok();
            return Ok(response_data);
        }

        // if command.data().starts_with(
        hprintln!("got not PIV: {:?}", &command.data()).ok();
        Err(Status::NotFound)
    }
}


//pub fn fake_piv(command: &mut MessageBuffer) {
//        // This is what we get from `piv-agent`
//        // raw APDU: 00 87 11 9A 26 7C 24 82 00 81 20 E6 57 78 FC E5 C5 D8 03 4F EA C9 17 27 D5 8A 40 54 5F BC 05 BC 6A CD 37 85 3B F5 E4 E2 A9 33 F2
//        //
//        // APDU: 00 87 11 9A 26
//        //      7C 24
//        //          // 82 = response, empty = "request for request"
//        //          82 00
//        //          // 81 = challenge, length 0x20 = 32 bytes
//        //          81 20
//        //              E6 57 78 FC E5 C5 D8 03 4F EA C9 17 27 D5 8A 40 54 5F BC 05 BC 6A CD 37 85 3B F5 E4 E2 A9 33 F2
//        //
//        // reponse length = 76 bytes
//        // SW: 7C 4A 82 48 30 46 02 21 00 C2 E4 D8 7E B4 4A F1 A7 71 DC F8 69 5C F5 CA BD 9A 71 C9 4F 16 FB B6 FF FF CC E2 1E D2 49 BE C8 02 21 00 BE 63 44 F3 33 CD D9 4E 1C CB 52 43 EB 1D 78 11 0E A2 AB E0 5A 3E A3 93 58 6C F0 82 28 E1 A2 B1
//        //      90 00
//        // GENERAL AUTHENTICATE => {
//        (0x00, 0x87, 0x11, 0x9a) => {
//            // P1 = alg = 0x11 = P256
//            // P2 = key = 0x9a = authentication (w/PIN)

//        }
//        // VERIFY => {
//        (0x00, 0x20, 0x00, 0x80) => {
//            // P2 = 0x80 = PIV  card application PIN

//            // APDU: 00 20 00 80 00
//            // SW: 63 C3
//            // APDU: 00 20 00 80 00
//            // SW: 63 C3
//            // APDU: 00 20 00 80 08 31 32 33 34 FF FF FF FF
//            // SW: 63 C2
//            match apdu.data().len() {
//                // case of missing data: used to read out retries
//                // - '63 CX' => X retries
//                // - '90 00' => no PIN set
//                0 => {
//                    command.clear();
//                    // 0x63 = verification failed, Cx => x = remaining tries
//                    command.extend_from_slice(&[0x63, 0xC3]).unwrap();
//                }
//                // shorter PINs are padded
//                8 => {
//                    // PIN "1234"
//                    if apdu.data() == [0x31, 0x32, 0x33, 0x34, 0xff, 0xff, 0xff, 0xff] {
//                        command.clear();
//                        command.extend_from_slice(OK).unwrap();
//                    } else {
//                        command.clear();
//                        // TODO: decrement PIN retries (here we "set" it to 2)
//                        command.extend_from_slice(&[0x63, 0xc2]).unwrap();
//                        // if retries = 0, then return '69 83'
//                    }
//                }
//                _ => {
//                    command.clear();
//                    // "incorrect parameter in command data field"
//                    command.extend_from_slice(&[0x6a, 0x80]).unwrap();
//                }
//            }
//        }
//        // 00000156 APDU: 00 A4 04 00 05 A0 00 00 03 08
//        // 00001032 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
//        //
//        // 00009280 APDU: 00 A4 04 00 05 A0 00 00 03 08
//        // 00001095 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
//        //
//        // 00000117 APDU: 00 FD 00 00 00
//        // 00001057 SW: 04 03 04 90 00
//        //
//        // 00000152 APDU: 00 A4 04 00 08 A0 00 00 05 27 20 01 01
//        // 00001154 SW: 04 03 04 01 05 00 05 0F 00 00 90 00
//        //
//        // 00000112 APDU: 00 01 10 00 00
//        // 00001010 SW: 00 52 F7 43 90 00
//        //
//        // 00000102 APDU: 00 A4 04 00 05 A0 00 00 03 08
//        // 00001426 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
//        _ => {
//            panic!("unhandled APDU (0x{:x}, 0x{:x}, 0x{:x}, 0x{:x}, {}), !",
//                cla, ins, p1, p2, le);
//        }
//    }
//}

// calling `yubikey readers`, response from NEO OTP+U2F+CCID
// 05808693 APDU: 00 A4 04 00 05 A0 00 00 03 08
// 00011103 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
// 00000145 APDU: 00 FD 00 00 00
// 00005749 SW: 01 00 04 90 00
// 00000131 APDU: 00 A4 04 00 08 A0 00 00 05 27 20 01 01
// 00013940 SW: 03 04 01 01 85 07 06 0F 00 00 90 00
// 00008731 APDU: 00 01 10 00 00
// 00008949 SW: 00 60 E8 4B 90 00
// 00000090 APDU: 00 A4 04 00 05 A0 00 00 03 08
// 00008148 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
// 00039651 APDU: 00 A4 04 00 05 A0 00 00 03 08
// 00008103 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
// 00000086 APDU: 00 FD 00 00 00
// 00006044 SW: 01 00 04 90 00
// 00000101 APDU: 00 A4 04 00 08 A0 00 00 05 27 20 01 01
// 00009155 SW: 03 04 01 01 85 07 06 0F 00 00 90 00
// 00000228 APDU: 00 01 10 00 00
// 00005829 SW: 00 60 E8 4B 90 00
// 00000094 APDU: 00 A4 04 00 05 A0 00 00 03 08
// 00008128 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
//
//
// 00003001 readerfactory.c:376:RFAddReader() Yubico YubiKey FIDO+CCID init failed.
//
// 03510021 APDU: 00 A4 04 00 05 A0 00 00 03 08
// 00001106 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
// --> 90 00 = OK
//
// 00000104 APDU: 00 FD 00 00 00
// 00000949 SW: 04 03 04 90 00
// --> ?!?! what is this `FD` command?!
//
// 00000141 APDU: 00 A4 04 00 08 A0 00 00 05 27 20 01 01
// 00001183 SW: 04 03 04 01 05 00 05 0F 00 00 90 00
//
// 00000489 APDU: 00 01 10 00 00
// 00000950 SW: 00 52 F7 43 90 00
// --> ?!?! what is this `01` command?!
//
// 00000156 APDU: 00 A4 04 00 05 A0 00 00 03 08
// 00001032 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
//
// 00009280 APDU: 00 A4 04 00 05 A0 00 00 03 08
// 00001095 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00
//
// 00000117 APDU: 00 FD 00 00 00
// 00001057 SW: 04 03 04 90 00
//
// 00000152 APDU: 00 A4 04 00 08 A0 00 00 05 27 20 01 01
// 00001154 SW: 04 03 04 01 05 00 05 0F 00 00 90 00
//
// 00000112 APDU: 00 01 10 00 00
// 00001010 SW: 00 52 F7 43 90 00
//
// 00000102 APDU: 00 A4 04 00 05 A0 00 00 03 08
// 00001426 SW: 61 11 4F 06 00 00 10 00 01 00 79 07 4F 05 A0 00 00 03 08 90 00

