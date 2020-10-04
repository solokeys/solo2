#![cfg_attr(not(test), no_std)]

pub mod constants;
pub mod state;

logging::add!(logger);
use logger::{blocking};

use core::convert::{TryFrom, TryInto};

use heapless::consts;
use iso7816::{
    Command, Instruction, Status,
    response::{
        Result as ResponseResult,
        Data as ResponseData,
    },
};
use apdu_dispatch::applet;
// use apdu_dispatch::{Aid, Applet, Result as applet::Result, applet::Response};
use trussed::Client as Trussed;
use trussed::{block, syscall};

use usbd_ccid::{
    // constants::*,
    der::Der,
    derp,
};

use constants::*;

pub struct App
{
    state: state::State,
    trussed: Trussed,
    // trussed: RefCell<Trussed>,
}

impl App
{
    pub fn new(
        trussed: Trussed,
    )
        -> Self
    {
        // seems like RefCell is not the right thing, we want something like `Rc` instead,
        // which can be cloned and injected into other parts of the App that use Trussed.
        // let trussed = RefCell::new(trussed);
        Self {
            // state: state::State::new(trussed.clone()),
            state: Default::default(),
            trussed,
        }
    }

    fn try_handle(&mut self, command: &Command) -> ResponseResult {

        // TEMP
        // blocking::dbg!(self.state.persistent(&mut self.trussed).timestamp(&mut self.trussed));

        // handle CLA
        // - command chaining not supported
        // - secure messaging not supported
        // - only channel zero supported
        // - ensure INS known to us

        let last_or_only = command.class().chain().last_or_only();

        // TODO: avoid owned copy?
        let owned_command = match self.state.runtime.chained_command.as_mut() {
            Some(command_so_far) => {
                // TODO: make sure the prefix matches, e.g. '00 DB 3F FF'
                command_so_far.data_mut().extend_from_slice(command.data()).unwrap();

                if last_or_only {
                    let total_command = command_so_far.clone();
                    self.state.runtime.chained_command = None;
                    total_command
                } else {
                    return Ok(Default::default());
                }
            }

            None => {
                if last_or_only {
                    // Command
                    command.clone()
                } else {
                    self.state.runtime.chained_command = Some(command.clone());
                    return Ok(Default::default());
                }
            }
        };

        let command = &owned_command;

        let class = command.class();

        if !class.secure_messaging().none() {
            return Err(Status::SecureMessagingNotSupported);
        }

        if class.channel() != Some(0) {
            return Err(Status::LogicalChannelNotSupported);
        }

        // blocking::info!("CLA = {:?}", &command.class()).ok();
        blocking::info!("INS = {:?}, P1 = {:X}, P2 = {:X}",
                  &command.instruction(),
                  command.p1, command.p2,
                  ).ok();
        // blocking::info!("extended = {:?}", command.extended).ok();

        // blocking::info!("INS = {:?}" &command.instruction()).ok();
        match command.instruction() {
            Instruction::GetData => self.get_data(command),
            Instruction::PutData => self.put_data(command),
            Instruction::Verify => self.verify(command),
            Instruction::ChangeReferenceData => self.change_reference_data(command),
            Instruction::GeneralAuthenticate => self.general_authenticate(command),
            Instruction::GenerateAsymmetricKeyPair => self.generate_asymmetric_keypair(command),

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

        let _alg = command.p1;
        let _slot = command.p2;
        let mut data = command.data().as_slice();

        // refine as we gain more capability
        if data.len() < 2 {
            return Err(Status::IncorrectDataParameter);
        }

        let tag = data[0];
        if tag != 0x7c {
            return Err(Status::IncorrectDataParameter);
        }

        if data[1] > 0x81 {
            panic!("unhandled >1B lengths");
        }
        if data[1] == 0x81 {
            data[2] as usize;
            data = &data[3..];
        } else {
            data[1] as usize; // ~158 for ssh ed25519 signatures (which have a ~150B commitment)
            data = &data[2..];
        };

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

        // // expect '81 20'
        // if !data.starts_with(&[0x81, 0x20]) {
        //     return Err(Status::IncorrectDataParameter);
        // }
        // data = &data[2..];

        // expect '81 81 96'
        // if !data.starts_with(&[0x81, 0x81, 0x96]) {
        if !data.starts_with(&[0x81, 0x81]) {
            return Err(Status::IncorrectDataParameter);
        }
        let len = data[2] as usize;
        data = &data[3..];

        // if data.len() != 32 {
        //     return Err(Status::IncorrectDataParameter);
        // }
        if data.len() != len {
            return Err(Status::IncorrectDataParameter);
        }

        let mechanism = trussed::types::Mechanism::Ed25519;
        let commitment = data; // 32B of data // 150B for ed25519
        // blocking::dbg!(commitment);
        let serialization = trussed::types::SignatureSerialization::Asn1Der; // ed25519 disregards

        blocking::info!("looking for keyreference").ok();
        let key_handle = match self.state.persistent(&mut self.trussed).keys.authentication_key {
            Some(key) => key,
            None => return Err(Status::KeyReferenceNotFound),
        };

        let signature = block!(self.trussed.sign(mechanism, key_handle, commitment, serialization).unwrap())
            .map_err(|error| {
                // NoSuchKey
                blocking::dbg!(error).ok();
                Status::UnspecifiedNonpersistentExecutionError }
            )?
            .signature;
        // blocking::dbg!(&signature);

        let mut der: Der<consts::U256> = Default::default();
        // 7c = Dynamic Authentication Template tag
        der.nested(0x7c, |der| {
            // 82 = response
            der.raw_tlv(0x82, &signature)
        }).unwrap();
        // blocking::dbg!(&der);

        let response_data: ResponseData = der.to_byte_buf();
        // blocking::dbg!(&response_data);
        return Ok(response_data);

        // blocking::dbg!("NOW WE SHOULD WORK");
        // Err(Status::FunctionNotSupported)
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
            blocking::dbg!(our_challenge, response).ok();
            return Err(Status::IncorrectDataParameter);
        }

        self.state.runtime.app_security_status.management_verified = true;

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

        let response_data: ResponseData = der.to_byte_buf();
        // blocking::dbg!(&response_data);
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

        let key = self.state.persistent(&mut self.trussed).keys.management_key;

        let challenge = syscall!(self.trussed.random_bytes(8)).bytes;
        let command_cache = state::AuthenticateManagement { challenge: challenge[..].try_into().unwrap() };
        self.state.runtime.command_cache = Some(state::CommandCache::AuthenticateManagement(command_cache));

        let encrypted_challenge = block!(self.trussed.encrypt_tdes(&key, &challenge).unwrap()).unwrap().ciphertext;

        let mut der: Der<consts::U12> = Default::default();
        // 7c = Dynamic Authentication Template tag
        der.nested(0x7c, |der| {
            // 80 = witness
            der.raw_tlv(0x80, &encrypted_challenge)
        }).unwrap();

        return Ok(der.to_byte_buf());

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

        // application PIN
        if command.p2 == 0x80 {
            let remaining_retries = self.state.persistent(&mut self.trussed).remaining_pin_retries();

            if remaining_retries == 0 {
                return Err(Status::OperationBlocked);
            }

            if command.data().len() != 16 {
                return Err(Status::IncorrectDataParameter);
            }

            let (old_pin, new_pin) = command.data().split_at(8);

            let old_pin = match state::Pin::try_new(old_pin) {
                Ok(pin) => pin,
                _ => return Err(Status::IncorrectDataParameter),
            };

            let new_pin = match state::Pin::try_new(new_pin) {
                Ok(pin) => pin,
                _ => return Err(Status::IncorrectDataParameter),
            };

            if !self.state.persistent(&mut self.trussed).verify_pin(&old_pin) {
                let remaining = self.state.persistent(&mut self.trussed).increment_consecutive_pin_mismatches(&mut self.trussed);
                self.state.runtime.app_security_status.pin_verified = false;
                return Err(Status::RemainingRetries(remaining));
            }

            self.state.persistent(&mut self.trussed).reset_consecutive_pin_mismatches(&mut self.trussed);
            self.state.persistent(&mut self.trussed).set_pin(&mut self.trussed, new_pin);
            self.state.runtime.app_security_status.pin_verified = true;
            return Ok(Default::default());
        }

        // PUK
        if command.p2 == 0x81 {
            let remaining_retries = self.state.persistent(&mut self.trussed).remaining_puk_retries();

            if remaining_retries == 0 {
                return Err(Status::OperationBlocked);
            }

            if command.data().len() != 16 {
                return Err(Status::IncorrectDataParameter);
            }

            let (old_puk, new_puk) = command.data().split_at(8);

            let old_puk = match state::Pin::try_new(old_puk) {
                Ok(puk) => puk,
                _ => return Err(Status::IncorrectDataParameter),
            };

            let new_puk = match state::Pin::try_new(new_puk) {
                Ok(puk) => puk,
                _ => return Err(Status::IncorrectDataParameter),
            };

            if !self.state.persistent(&mut self.trussed).verify_puk(&old_puk) {
                let remaining = self.state.persistent(&mut self.trussed).increment_consecutive_puk_mismatches(&mut self.trussed);
                self.state.runtime.app_security_status.puk_verified = false;
                return Err(Status::RemainingRetries(remaining));
            }

            self.state.persistent(&mut self.trussed).reset_consecutive_puk_mismatches(&mut self.trussed);
            self.state.persistent(&mut self.trussed).set_puk(&mut self.trussed, new_puk);
            self.state.runtime.app_security_status.puk_verified = true;
            return Ok(Default::default());
        }


        Err(Status::KeyReferenceNotFound)
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
            self.state.runtime.app_security_status.pin_verified = false;
            Err(Status::RemainingRetries(remaining))
        }
    }

    fn generate_asymmetric_keypair(&mut self, command: &Command) -> ResponseResult {
        if !self.state.runtime.app_security_status.management_verified {
            return Err(Status::SecurityStatusNotSatisfied);
        }

        if command.p1 != 0x00 {
            return Err(Status::IncorrectP1OrP2Parameter);
        }

        if command.p2 != 0x9a {
            // TODO: make more general
            return Err(Status::FunctionNotSupported);
        }

        // example: 00 47 00 9A 0B
        //   AC 09
        //      # P256
        //      80 01 11
        //      # 0xAA = Yubico extension (of course...), PinPolicy, 0x2 =
        //      AA 01 02
        //      # 0xAB = Yubico extension (of course...), TouchPolicy, 0x2 =
        //      AB 01 02
        //
        // var touchPolicyMap = map[TouchPolicy]byte{
        //     TouchPolicyNever:  0x01,
        //     TouchPolicyAlways: 0x02,
        //     TouchPolicyCached: 0x03,
        // }

        // var pinPolicyMap = map[PINPolicy]byte{
        //     PINPolicyNever:  0x01,
        //     PINPolicyOnce:   0x02,
        //     PINPolicyAlways: 0x03,
        // }

        // TODO: iterate on this, don't expect tags..
        let input = derp::Input::from(&command.data());
        // let (mechanism, parameter) = input.read_all(derp::Error::Read, |input| {
        let (mechanism, _pin_policy, _touch_policy) = input.read_all(derp::Error::Read, |input| {
            derp::nested(input, 0xac, |input| {
                let mechanism = derp::expect_tag_and_get_value(input, 0x80)?;
                // let parameter = derp::expect_tag_and_get_value(input, 0x81)?;
                let pin_policy = derp::expect_tag_and_get_value(input, 0xaa)?;
                let touch_policy = derp::expect_tag_and_get_value(input, 0xab)?;
                // Ok((mechanism.as_slice_less_safe(), parameter.as_slice_less_safe()))
                Ok((
                    mechanism.as_slice_less_safe(),
                    pin_policy.as_slice_less_safe(),
                    touch_policy.as_slice_less_safe(),
                ))
            })
        }).map_err(|e| {
                blocking::info!("error parsing GenerateAsymmetricKeypair: {:?}", &e).ok();
                Status::IncorrectDataParameter
        })?;

        // if mechanism != &[0x11] {
        // HA! patch in Ed25519
        if mechanism != &[0x22] {
            return Err(Status::InstructionNotSupportedOrInvalid);
        }

        // ble policy

        if let Some(key) = self.state.persistent(&mut self.trussed).keys.authentication_key {
            syscall!(self.trussed.delete(key));
        }

        // let key = syscall!(self.trussed.generate_p256_private_key(
        // let key = syscall!(self.trussed.generate_p256_private_key(
        let key = syscall!(self.trussed.generate_ed25519_private_key(
            trussed::types::StorageLocation::Internal,
        )).key;


        // // TEMP
        // let mechanism = trussed::types::Mechanism::P256Prehashed;
        // let mechanism = trussed::types::Mechanism::P256;
        // let commitment = &[37u8; 32];
        // // blocking::dbg!(commitment);
        // let serialization = trussed::types::SignatureSerialization::Asn1Der;
        // // blocking::dbg!(&key);
        // let signature = block!(self.trussed.sign(mechanism, key.clone(), commitment, serialization).map_err(|e| {
        //     blocking::dbg!(e);
        //     e
        // }).unwrap())
        //     .map_err(|error| {
        //         // NoSuchKey
        //         blocking::dbg!(error);
        //         Status::UnspecifiedNonpersistentExecutionError }
        //     )?
        //     .signature;
        // blocking::dbg!(&signature);

        self.state.persistent(&mut self.trussed).keys.authentication_key = Some(key);
        self.state.persistent(&mut self.trussed).save(&mut self.trussed);

        // let public_key = syscall!(self.trussed.derive_p256_public_key(
        let public_key = syscall!(self.trussed.derive_ed25519_public_key(
            &key,
            trussed::types::StorageLocation::Volatile,
        )).key;

        let serialized_public_key = syscall!(self.trussed.serialize_key(
            // trussed::types::Mechanism::P256,
            trussed::types::Mechanism::Ed25519,
            public_key.clone(),
            trussed::types::KeySerialization::Raw,
        )).serialized_key;

        // blocking::info!("supposed SEC1 pubkey, len {}: {:X?}", serialized_public_key.len(), &serialized_public_key).ok();

        // P256 SEC1 has 65 bytes, Ed25519 pubkeys have 32
        // let l2 = 65;
        let l2 = 32;
        let l1 = l2 + 2;
        let mut data = ResponseData::from_slice(&[0x7f, 0x49, l1, 0x86, l2]).unwrap();
        // data.extend_from_slice(&[0x04]).unwrap();
        data.extend_from_slice(&serialized_public_key).unwrap();

        Ok(data)
    }

    fn put_data(&mut self, command: &Command) -> ResponseResult {
        blocking::info!("PutData").ok();
        if command.p1 != 0x3f || command.p2 != 0xff {
            return Err(Status::IncorrectP1OrP2Parameter);
        }

        // if !self.state.runtime.app_security_status.management_verified {
        //     return Err(Status::SecurityStatusNotSatisfied);
        // }

        // # PutData
        // 00 DB 3F FF 23
        //    # data object: 5FC109
        //    5C 03 5F C1 09
        //    # data:
        //    53 1C
        //       # actual data
        //       88 1A 89 18 AA 81 D5 48 A5 EC 26 01 60 BA 06 F6 EC 3B B6 05 00 2E B6 3D 4B 28 7F 86
        //

        let input = derp::Input::from(&command.data());
        let (data_object, data) = input.read_all(derp::Error::Read, |input| {
            let data_object = derp::expect_tag_and_get_value(input, 0x5c)?;
            let data = derp::expect_tag_and_get_value(input, 0x53)?;
            Ok((data_object.as_slice_less_safe(), data.as_slice_less_safe()))
        // }).unwrap();
        }).map_err(|e| {
                blocking::info!("error parsing PutData: {:?}", &e).ok();
                Status::IncorrectDataParameter
        })?;

        // blocking::info!("PutData in {:?}: {:?}", data_object, data).ok();

        if data_object == &[0x5f, 0xc1, 0x09] {
            // "Printed Information", supposedly
            // Yubico uses this to store its "Metadata"
            //
            // 88 1A
            //    89 18
            //       # we see here the raw management key? amazing XD
            //       AA 81 D5 48 A5 EC 26 01 60 BA 06 F6 EC 3B B6 05 00 2E B6 3D 4B 28 7F 86

            // TODO: use smarter quota rule, actual data sent is 28B
            if data.len() >= 512 {
                return Err(Status::UnspecifiedCheckingError);
            }

            block!(self.trussed.write_file(
                trussed::types::StorageLocation::Internal,
                trussed::types::PathBuf::from(b"printed-information"),
                trussed::types::Message::from_slice(data).unwrap(),
                None,
            ).unwrap()).map_err(|_| Status::NotEnoughMemory)?;

            return Ok(Default::default());
        }

        if data_object == &[0x5f, 0xc1, 0x05] {
            // "X.509 Certificate for PIV Authentication", supposedly
            // IOW, the cert for "authentication key"
            // Yubico uses this to store its "Metadata"
            //
            // 88 1A
            //    89 18
            //       # we see here the raw management key? amazing XD
            //       AA 81 D5 48 A5 EC 26 01 60 BA 06 F6 EC 3B B6 05 00 2E B6 3D 4B 28 7F 86

            // TODO: use smarter quota rule, actual data sent is 28B
            if data.len() >= 512 {
                return Err(Status::UnspecifiedCheckingError);
            }

            block!(self.trussed.write_file(
                trussed::types::StorageLocation::Internal,
                trussed::types::PathBuf::from(b"authentication-key.x5c"),
                trussed::types::Message::from_slice(data).unwrap(),
                None,
            ).unwrap()).map_err(|_| Status::NotEnoughMemory)?;

            return Ok(Default::default());
        }

        Err(Status::IncorrectDataParameter)
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
        blocking::info!("looking up {:?}", data).ok();

        // TODO: check security status, else return Status::SecurityStatusNotSatisfied

        // Table 3, Part 1, SP 800-73-4
        // https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-73-4.pdf#page=30
        match data {
            DataObjects::DiscoveryObject => {
                // Err(Status::InstructionNotSupportedOrInvalid)
                let data = ResponseData::from_slice(DISCOVERY_OBJECT).unwrap();
                Ok(data)
                // todo!("discovery object"),
            }

            DataObjects::BiometricInformationTemplate => {
                Err(Status::InstructionNotSupportedOrInvalid)
                // todo!("biometric information template"),
            }

            // '5FC1 02' (351B)
            DataObjects::CardHolderUniqueIdentifier => {
                // pivy: https://git.io/JfzBo
                // https://www.idmanagement.gov/wp-content/uploads/sites/1171/uploads/TIG_SCEPACS_v2.3.pdf
                let mut der = Der::<consts::U1024>::default();
                der.nested(0x53, |der| {
                    // der.raw_tlv(0x30, FASC_N)?; // pivy: 26B, TIG: 25B
                    der.raw_tlv(0x30, &[0x99, 0x99])?; // 9999 = non-federal; pivy: 26B, TIG: 25B
                    // der.raw_tlv(0x34, DUNS)?; // ? - pivy skips
                    der.raw_tlv(0x34, GUID)?; // 16B type 1,2,4 UUID
                    // der.raw_tlv(0x35, EXPIRATION_DATE)?; // [u8; 8], YYYYMMDD
                    der.raw_tlv(0x35, b"22220101")?; // [u8; 8], YYYYMMDD
                    // der.raw_tlv(0x36, CARDHOLDER_UUID)?; // 16B, like GUID
                    // der.raw_tlv(0x3E, SIGNATURE)?; // ? - pivy only checks for non-zero entry
                    der.raw_tlv(0x3E, b" ")?; // ? - pivy only checks for non-zero entry
                    Ok(())
                }).unwrap();

                Ok(der.to_byte_buf())
            }

            // '5FC1 05' (351B)
            DataObjects::X509CertificateForPivAuthentication => {
                // return Err(Status::NotFound);

                // blocking::info!("loading 9a cert").ok();
                // it seems like fetching this certificate is the way Filo's agent decides
                // whether the key is "already setup":
                // https://github.com/FiloSottile/yubikey-agent/blob/8781bc0082db5d35712a2244e3ab3086f415dd59/setup.go#L69-L70
                let data = block!(self.trussed.read_file(
                    trussed::types::StorageLocation::Internal,
                    trussed::types::PathBuf::from(b"authentication-key.x5c"),
                ).unwrap()).map_err(|_| {
                    // blocking::info!("error loading: {:?}", &e).ok();
                    Status::NotFound
                } )?.data;
                // blocking::info!("got the data: {:?}", &data).ok();

                let mut der: Der<consts::U1024> = Default::default();
                der.raw_tlv(0x53, &data).unwrap();
                Ok(der.to_byte_buf())
            }

            // '5F FF01' (754B)
            YubicoObjects::AttestationCertificate => {
                let data = ResponseData::from_slice(YUBICO_ATTESTATION_CERTIFICATE).unwrap();
                Ok(data)
            }

            _ => return Err(Status::NotFound),
        }
    }

    fn yubico_piv_extension(&mut self, command: &Command, instruction: YubicoPivExtension) -> ResponseResult {
        blocking::info!("yubico extension: {:?}", &instruction).ok();
        match instruction {
            YubicoPivExtension::GetSerial => {
                // make up a 4-byte serial
                let data = ResponseData::from_slice(
                    &[0x00, 0x52, 0xf7, 0x43]).unwrap();
                Ok(data)
            }

            YubicoPivExtension::GetVersion => {
                // make up a version, be >= 5.0.0
                let data = ResponseData::from_slice(
                    &[0x06, 0x06, 0x06]).unwrap();
                Ok(data)
            }

            YubicoPivExtension::Attest => {
                if command.p2 != 0x00 {
                    return Err(Status::IncorrectP1OrP2Parameter);
                }

                let slot = command.p1;

                if slot == 0x9a {
                    let data = ResponseData::from_slice(YUBICO_ATTESTATION_CERTIFICATE_FOR_9A).unwrap();
                    return Ok(data);
                }

                Err(Status::FunctionNotSupported)
            }

            YubicoPivExtension::Reset => {
                if command.p1 != 0x00 || command.p2 != 0x00 {
                    return Err(Status::IncorrectP1OrP2Parameter);
                }

                // TODO: find out what all needs resetting :)
                self.state.persistent(&mut self.trussed).reset_pin(&mut self.trussed);
                self.state.persistent(&mut self.trussed).reset_puk(&mut self.trussed);
                self.state.persistent(&mut self.trussed).reset_management_key(&mut self.trussed);
                self.state.runtime.app_security_status.pin_verified = false;
                self.state.runtime.app_security_status.puk_verified = false;
                self.state.runtime.app_security_status.management_verified = false;

                block!(self.trussed.remove_file(
                    trussed::types::StorageLocation::Internal,
                    trussed::types::PathBuf::from(b"printed-information"),
                ).unwrap()).ok();

                block!(self.trussed.remove_file(
                    trussed::types::StorageLocation::Internal,
                    trussed::types::PathBuf::from(b"authentication-key.x5c"),
                ).unwrap()).ok();

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

}


impl applet::Aid for App {

    fn aid(&self) -> &'static [u8] {
        &constants::PIV_AID
    }

    fn right_truncated_length(&self) -> usize {
        11
    }
}


impl applet::Applet for App {
    fn select(&mut self, _apdu: &Command) -> applet::Result {
        let mut der: Der<consts::U256> = Default::default();
        der.nested(0x61, |der| {
            // Application identifier of application:
            // -> PIX (without RID, with version)
            der.raw_tlv(0x4f, &PIV_PIX)?;

            // Application label:
            // "Text describing the application; e.g., for use on a man-machine interface."
            der.raw_tlv(0x50, APPLICATION_LABEL)?;

            // Uniform resource locator:
            // "Reference to the specification describing the application."
            der.raw_tlv2(0x5F50, APPLICATION_URL)?;

            // Cryptographic algorithms supported:
            // "Cryptographic algorithm identifier template. See Table 5."
            der.nested(0xAC, |der| {
                // 0x80: Cryptographic algorithm identifier
                // "For values see [SP800-78, Table 6-2]"

                // 0C: AES-256
                der.raw_tlv(0x80, &[0x0C])?;
                // 11: ECC-P256
                der.raw_tlv(0x80, &[0x11])?;

                // 22 (non-standard!): Ed25519
                der.raw_tlv(0x80, &[0x22])?;

                // mandatory "Object identifier" with value set to 0x00
                der.raw_tlv(0x06, &[0x00])
            })?;

            // Coexistent tag allocation authority
            der.nested(0x79, |der| {
                // Application identifier
                der.raw_tlv(0x4f, NIST_RID)
            // })?;
            })
        }).unwrap();

        return Ok(applet::Response::Respond(der.to_byte_buf()));
    }

    fn deselect(&mut self) {}

    fn call(&mut self, _type: applet::InterfaceType, apdu: &Command) -> applet::Result {
        match self.try_handle(apdu) {
            Ok(data) => {
                Ok(applet::Response::Respond(data))
            }
            Err(status) => {
                Err(status)
            }
        }
    }
}
