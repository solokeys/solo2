#![cfg_attr(not(test), no_std)]

pub mod constants;
pub mod state;

use core::convert::{TryFrom, TryInto};

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

#[macro_use]
macro_rules! block {
    ($future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $future_result;
        loop {
            match future_result.poll() {
                core::task::Poll::Ready(result) => { break result; },
                core::task::Poll::Pending => {},
            }
        }
    }}
}

#[macro_use]
macro_rules! syscall {
    ($pre_future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $pre_future_result.expect("no client error");
        loop {
            match future_result.poll() {
                // core::task::Poll::Ready(result) => { break result.expect("no errors"); },
                core::task::Poll::Ready(result) => { break result.unwrap(); },
                core::task::Poll::Pending => {},
            }
        }
    }}
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
        // dbg!(self.state.persistent(&mut self.trussed).timestamp(&mut self.trussed));

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

        // hprintln!("CLA = {:?}", &command.class()).ok();
        hprintln!("INS = {:?}, P1 = {:X}, P2 = {:X}",
                  &command.instruction(),
                  command.p1, command.p2,
                  ).ok();
        // hprintln!("extended = {:?}", command.extended).ok();

        // hprintln!("INS = {:?}" &command.instruction()).ok();
        match command.instruction() {
            Instruction::Select => self.select(command),
            Instruction::GetData => self.get_data(command),
            Instruction::Verify => self.verify(command),
            Instruction::ChangeReferenceData => self.change_reference_data(command),
            Instruction::GeneralAuthenticate => self.general_authenticate(command),

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

    // SP 800-73-4, Part 2, Section 3.2.4
    // https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-73-4.pdf#page=92
    //
    // General use:
    // - PIV authn keys (9A, 9B, 9E):
    //   - card/app to client (INTERNAL)
    //   - entity to card (EXTERNAL)
    //   - mutual card/external (MUTUAL)
    // - Signature key (9C): => Appendix A.4
    //   - signing data hashed off card
    // - Management key (9D, retired 82-95): => Appendix A.5
    //   - key establishment schems in SP 800-78 (ECDH)
    // - PIV secure messaging key (04, alg 27, 2E)
    //
    // Data field tags:
    // - 80 witness
    // - 81 challenge
    // - 82 response
    // - 83 exponentiation
    //
    // Request for requests:
    // - '80 00' returns '80 TL <encrypted random>'
    // - '81 00' returns '81 TL <random>'
    //
    // Errors:
    // - 9000, 61XX for success
    // - 6982 security status
    // - 6A80, 6A86 for data, P1/P2 issue
    fn general_authenticate(&mut self, command: &Command) -> ResponseResult {

        // For "SSH", we need implement A.4.2 in SP-800-73-4 Part 2, ECDSA signatures
        //
        // ins = 87 = general authenticate
        // p1 = 11 = alg P256
        // p2 = 9a = keyref "PIV authentication"
        // 00 87 11 9A 26
        //     # 7c = specified template
        //     7C 24
        //         # 82 = response, 00 = "request for request"
        //         82 00
        //         # 81 = challenge
        //         81 20
        //             # 32B nonce
        //             95 AE 21 F9 5E 00 01 E6 23 27 F4 FD A5 05 F1 F5 B7 95 0F 11 75 BC 4D A2 06 B1 00 6B DA 90 C3 3A
        //
        // expected response: "7C L1 82 L2 SEQ(INT r, INT s)"

        let alg = command.p1;
        let key = command.p2; // should we use Yubico's "slot" terminology? i don't like it
        let mut data = command.data().as_ref();

        // refine as we gain more capability
        if data.len() < 2 {
            return Err(Status::IncorrectDataParameter);
        }

        let tag = data[0];
        if tag != 0x7c {
            return Err(Status::IncorrectDataParameter);
        }

        let len = data[1] as usize;
        data = &data[2..];

        // step 1 of piv-go/ykAuthenticate
        // https://github.com/go-piv/piv-go/blob/d5ec95eb3bec9c20d60611fb77b7caeed7d886b6/piv/piv.go#L359-L384
        if data.starts_with(&[0x80, 0x00]) {
            // "request for witness"
            // hint that this is an attempt to SetManagementKey
            data = &data[2..];
            return self.request_for_witness(command, data);
        }

        // step 2 of piv-go/ykAuthenticate
        // https://github.com/go-piv/piv-go/blob/d5ec95eb3bec9c20d60611fb77b7caeed7d886b6/piv/piv.go#L415-L420
        if data.starts_with(&[0x80, 0x08]) {
            data = &data[2..];
            return self.request_for_challenge(command, data);
        }

        // expect '82 00'
        if !data.starts_with(&[0x82, 0x00]) {
            return Err(Status::IncorrectDataParameter);
        }
        data = &data[2..];

        // expect '81 20'
        if !data.starts_with(&[0x81, 0x20]) {
            return Err(Status::IncorrectDataParameter);
        }
        data = &data[2..];

        if data.len() != 32 {
            return Err(Status::IncorrectDataParameter);
        }

        let mechanism = trussed::types::Mechanism::P256Prehashed;
        let commitment = data; // 32B of data
        // dbg!(commitment);
        let serialization = trussed::types::SignatureSerialization::Asn1Der;

        let key_handle = trussed::types::ObjectHandle { object_id: trussed::types::UniqueId::try_from_hex(
            b"1234567890abcdef1234567890abcdef"
        ).unwrap() };
        // dbg!(key_handle);

        let signature = block!(self.trussed.sign(mechanism, key_handle, commitment, serialization).unwrap())
            .map_err(|error| {
                // NoSuchKey
                dbg!(error);
                Status::UnspecifiedNonpersistentExecutionError }
            )?
            .signature;
        dbg!(signature);

        dbg!("NOW WE SHOULD WORK");
        Err(Status::FunctionNotSupported)
    }

    fn request_for_challenge(&mut self, command: &Command, remaining_data: &[u8]) -> ResponseResult {
        // - data is of the form
        //     00 87 03 9B 16 7C 14 80 08 99 6D 71 40 E7 05 DF 7F 81 08 6E EF 9C 02 00 69 73 E8
        // - remaining data contains <decrypted challenge> 81 08 <encrypted counter challenge>
        // - we must a) verify the decrypted challenge, b) decrypt the counter challenge

        if command.p1 != 0x03 || command.p2 != 0x9b {
            return Err(Status::IncorrectP1OrP2Parameter);
        }

        if remaining_data.len() != 8 + 2 + 8 {
            return Err(Status::IncorrectDataParameter);
        }

        // A) verify decrypted challenge
        let (response, data) = remaining_data.split_at(8);

        use state::{AuthenticateManagement, CommandCache};
        let our_challenge = match self.state.runtime.command_cache {
            Some(CommandCache::AuthenticateManagement(AuthenticateManagement { challenge } ))
                => challenge,
            _ => { return Err(Status::InstructionNotSupportedOrInvalid); }
        };
        // no retries ;)
        self.state.runtime.command_cache = None;

        if &our_challenge != response {
            dbg!(our_challenge, response);
            return Err(Status::IncorrectDataParameter);
        }

        // TODO: actually store it and verify it
        // self.state.runtime.app_security_status.management_verified = true;

        // B) encrypt their challenge
        let (header, challenge) = data.split_at(2);
        if header != &[0x81, 0x08] {
            return Err(Status::IncorrectDataParameter);
        }

        let key = self.state.persistent(&mut self.trussed).keys.management_key;

        let encrypted_challenge = syscall!(self.trussed.encrypt_tdes(&key, &challenge)).ciphertext;

        let mut der: Der<consts::U12> = Default::default();
        // 7c = Dynamic Authentication Template tag
        der.nested(0x7c, |der| {
            // 82 = response
            der.raw_tlv(0x82, &encrypted_challenge)
        }).unwrap();

        let response_data: ResponseData = der.try_convert_into().unwrap();
        dbg!(&response_data);
        return Ok(response_data);
    }

    fn request_for_witness(&mut self, command: &Command, remaining_data: &[u8]) -> ResponseResult {
        // invariants: parsed data was '7C L1 80 00' + remaining_data

        if command.p1 != 0x03 || command.p2 != 0x9b {
            return Err(Status::IncorrectP1OrP2Parameter);
        }

        if !remaining_data.is_empty() {
            return Err(Status::IncorrectDataParameter);
        }

        hprintln!("l").ok();
        let key = self.state.persistent(&mut self.trussed).keys.management_key;
        hprintln!("s").ok();

        let challenge = syscall!(self.trussed.random_bytes(8)).bytes;
        hprintln!("b").ok();
        let command_cache = state::AuthenticateManagement { challenge: challenge[..].try_into().unwrap() };
        hprintln!("a").ok();
        self.state.runtime.command_cache = Some(state::CommandCache::AuthenticateManagement(command_cache));

        hprintln!("e").ok();
        let encrypted_challenge = block!(self.trussed.encrypt_tdes(&key, &challenge).unwrap()).unwrap().ciphertext;
        hprintln!("f").ok();

        let mut der: Der<consts::U12> = Default::default();
        // 7c = Dynamic Authentication Template tag
        der.nested(0x7c, |der| {
            // 80 = witness
            der.raw_tlv(0x80, &encrypted_challenge)
        }).unwrap();

        let response_data: ResponseData = der.try_convert_into().unwrap();
        hprintln!("challenge data: {:?}", &response_data).ok();
        return Ok(response_data);

    }

    fn change_reference_data(&mut self, command: &Command) -> ResponseResult {
        // The way `piv-go` blocks PUK (which it needs to do because Yubikeys only
        // allow their Reset if PIN+PUK are blocked) is that it sends "change PUK"
        // with random (i.e. incorrect) PUK listed as both old and new PUK.
        //
        // 00 24 00 81 10
        //       32 38 36 34 31 39 30 36 32 38 36 34 31 39 30 36
        //
        // For now, we don't support PUK, so we can just return "Blocked" directly
        // if the key reference in P2 is '81' = PUK

        if command.p2 == 0x81 {
            return Err(Status::OperationBlocked);
        }

        Err(Status::FunctionNotSupported)
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
                // it seems like fetching this certificate is the way Filo's agent decides
                // whether the key is "already setup":
                // https://github.com/FiloSottile/yubikey-agent/blob/8781bc0082db5d35712a2244e3ab3086f415dd59/setup.go#L69-L70

                Err(Status::NotFound)
                // let data = ResponseData::try_from_slice(YUBICO_PIV_AUTHENTICATION_CERTIFICATE).unwrap();
                // Ok(data)
            }

            // '5F FF01' (754B)
            YubicoObjects::AttestationCertificate => {
                let data = ResponseData::try_from_slice(YUBICO_ATTESTATION_CERTIFICATE).unwrap();
                Ok(data)
            }

            _ => return Err(Status::NotFound),
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

            YubicoPivExtension::Reset => {
                if command.p1 != 0x00 || command.p2 != 0x00 {
                    return Err(Status::IncorrectP1OrP2Parameter);
                }

                // TODO: find out what all needs resetting :)
                Ok(Default::default())
            }

            YubicoPivExtension::SetManagementKey => {
                // cmd := apdu{
                //     instruction: insSetMGMKey,
                //     param1:      0xff,
                //     param2:      0xff,
                //     data: append([]byte{
                //         alg3DES, keyCardManagement, 24,
                //     }, key[:]...),
                // }
                // TODO check we are authenticated with old management key
                if command.p1 != 0xff || (command.p2 != 0xff && command.p2 != 0xfe) {
                    return Err(Status::IncorrectP1OrP2Parameter);
                }

                let data = &command.data();

                // example:  03 9B 18
                //      B0 20 7A 20 DC 39 0B 1B A5 56 CC EB 8D CE 7A 8A C8 23 E6 F5 0D 89 17 AA
                if data.len() != 3 + 24 {
                    return Err(Status::IncorrectDataParameter);
                }
                let (prefix, new_management_key) = data.split_at(3);
                if prefix != &[0x03, 0x9b, 0x18] {
                    return Err(Status::IncorrectDataParameter);
                }
                let new_management_key: [u8; 24] = new_management_key.try_into().unwrap();
                self.state.persistent(&mut self.trussed).set_management_key(&mut self.trussed, &new_management_key);

                Ok(Default::default())
            }

            _ => Err(Status::FunctionNotSupported),
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

