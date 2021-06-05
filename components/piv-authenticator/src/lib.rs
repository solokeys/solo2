#![cfg_attr(not(test), no_std)]

#[macro_use]
extern crate delog;
generate_macros!();

#[macro_use(hex)]
extern crate hex_literal;

pub mod commands;
pub use commands::Command;
pub mod constants;
pub mod state;
pub mod derp;
pub mod piv_types;
pub use piv_types::{Pin, Puk};

use core::convert::{TryFrom, TryInto};

use flexiber::EncodableHeapless;
use iso7816::{
    Instruction, Status,
};
use apdu_dispatch::{Command as IsoCommand, response};
use trussed::client;
use trussed::{syscall, try_syscall};

use constants::*;

pub type Result = iso7816::Result<()>;

pub struct Authenticator<T>
{
    state: state::State,
    trussed: T,
    // trussed: RefCell<Trussed>,
}

impl<T> Authenticator<T>
where
    T: client::Client + client::Ed255 + client::Tdes,
{
    pub fn new(
        trussed: T,
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

    // TODO: we'd like to listen on multiple AIDs.
    // The way apdu-dispatch currently works, this would deselect, resetting security indicators.
    pub fn deselect(&mut self) {
    }

    pub fn select(&mut self, _apdu: &IsoCommand, reply: &mut response::Data) -> Result {
        use piv_types::Algorithms::*;

        let application_property_template = piv_types::ApplicationPropertyTemplate::default()
            .with_application_label(APPLICATION_LABEL)
            .with_application_url(APPLICATION_URL)
            .with_supported_cryptographic_algorithms(&[
                Aes256,
                P256,
                Ed255,
            ]);

        application_property_template
            .encode_to_heapless_vec(reply)
            .unwrap();
        Ok(())
    }

    pub fn respond(&mut self, command: &IsoCommand, reply: &mut response::Data) -> Result {
        let last_or_only = command.class().chain().last_or_only();

        // TODO: avoid owned copy?
        let entire_command = match self.state.runtime.chained_command.as_mut() {
            Some(command_so_far) => {
                // TODO: make sure the header matches, e.g. '00 DB 3F FF'
                command_so_far.data_mut().extend_from_slice(command.data()).unwrap();

                if last_or_only {
                    let entire_command = command_so_far.clone();
                    self.state.runtime.chained_command = None;
                    entire_command
                } else {
                    return Ok(Default::default());
                }
            }

            None => {
                if last_or_only {
                    // IsoCommand
                    command.clone()
                } else {
                    self.state.runtime.chained_command = Some(command.clone());
                    return Ok(Default::default());
                }
            }
        };

        // parse Iso7816Command as PivCommand
        let command: Command = (&entire_command).try_into()?;

        match command {
            Command::Verify(verify) => self.verify(verify),
            Command::ChangeReference(change_reference) => self.change_reference(change_reference),
            _ => todo!(),
        }
    }

    // maybe reserve this for the case VerifyLogin::PivPin?
    pub fn login(&mut self, login: commands::VerifyLogin) -> Result {
        if let commands::VerifyLogin::PivPin(pin) = login {

            // the actual PIN verification
            let mut persistent_state = self.state.persistent(&mut self.trussed);

            if persistent_state.remaining_pin_retries() == 0 {
                return Err(Status::OperationBlocked);
            }

            if persistent_state.verify_pin(&pin) {
                persistent_state.reset_consecutive_pin_mismatches();
                self.state.runtime.app_security_status.pin_verified = true;
                Ok(())

            } else {
                let remaining = persistent_state.increment_consecutive_pin_mismatches();
                // should we logout here?
                self.state.runtime.app_security_status.pin_verified = false;
                Err(Status::RemainingRetries(remaining))
            }
        } else {
            Err(Status::FunctionNotSupported)
        }
    }

    pub fn verify(&mut self, command: commands::Verify) -> Result {
        use commands::Verify;
        match command {
            Verify::Login(login) => self.login(login),

            Verify::Logout(_) => {
                self.state.runtime.app_security_status.pin_verified = false;
                Ok(())
            }

            Verify::Status(key_reference) => {
                if key_reference != commands::VerifyKeyReference::PivPin {
                    return Err(Status::FunctionNotSupported);
                }
                if self.state.runtime.app_security_status.pin_verified {
                    return Ok(())
                } else {
                    let retries = self.state.persistent(&mut self.trussed).remaining_pin_retries();
                    return Err(Status::RemainingRetries(retries));
                }
            }
        }
    }

    pub fn change_reference(&mut self, command: commands::ChangeReference) -> Result {
        use commands::ChangeReference;
        match command {
            ChangeReference::ChangePin { old_pin, new_pin } => self.change_pin(old_pin, new_pin),
            ChangeReference::ChangePuk { old_puk, new_puk } => self.change_puk(old_puk, new_puk),
        }
    }

    pub fn change_pin(&mut self, old_pin: commands::Pin, new_pin: commands::Pin) -> Result {
        let mut persistent_state = self.state.persistent(&mut self.trussed);
        if persistent_state.remaining_pin_retries() == 0 {
            return Err(Status::OperationBlocked);
        }

        if !persistent_state.verify_pin(&old_pin) {
            let remaining = persistent_state.increment_consecutive_pin_mismatches();
            self.state.runtime.app_security_status.pin_verified = false;
            return Err(Status::RemainingRetries(remaining));
        }

        persistent_state.reset_consecutive_pin_mismatches();
        persistent_state.set_pin(new_pin);
        self.state.runtime.app_security_status.pin_verified = true;
        return Ok(Default::default());
    }

    pub fn change_puk(&mut self, old_puk: commands::Puk, new_puk: commands::Puk) -> Result {
        let mut persistent_state = self.state.persistent(&mut self.trussed);
        if persistent_state.remaining_puk_retries() == 0 {
            return Err(Status::OperationBlocked);
        }

        if !persistent_state.verify_puk(&old_puk) {
            let remaining = persistent_state.increment_consecutive_puk_mismatches();
            self.state.runtime.app_security_status.puk_verified = false;
            return Err(Status::RemainingRetries(remaining));
        }

        persistent_state.reset_consecutive_puk_mismatches();
        persistent_state.set_puk(new_puk);
        self.state.runtime.app_security_status.puk_verified = true;
        return Ok(Default::default());
    }

    // pub fn old_respond(&mut self, command: &IsoCommand, reply: &mut response::Data) -> Result {

    //     // TEMP
    //     // blocking::dbg!(self.state.persistent(&mut self.trussed).timestamp(&mut self.trussed));

    //     // handle CLA
    //     // - command chaining not supported
    //     // - secure messaging not supported
    //     // - only channel zero supported
    //     // - ensure INS known to us

    //     let last_or_only = command.class().chain().last_or_only();

    //     // TODO: avoid owned copy?
    //     let owned_command = match self.state.runtime.chained_command.as_mut() {
    //         Some(command_so_far) => {
    //             // TODO: make sure the prefix matches, e.g. '00 DB 3F FF'
    //             command_so_far.data_mut().extend_from_slice(command.data()).unwrap();

    //             if last_or_only {
    //                 let total_command = command_so_far.clone();
    //                 self.state.runtime.chained_command = None;
    //                 total_command
    //             } else {
    //                 return Ok(Default::default());
    //             }
    //         }

    //         None => {
    //             if last_or_only {
    //                 // IsoCommand
    //                 command.clone()
    //             } else {
    //                 self.state.runtime.chained_command = Some(command.clone());
    //                 return Ok(Default::default());
    //             }
    //         }
    //     };

    //     let command = &owned_command;

    //     let class = command.class();

    //     if !class.secure_messaging().none() {
    //         return Err(Status::SecureMessagingNotSupported);
    //     }

    //     if class.channel() != Some(0) {
    //         return Err(Status::LogicalChannelNotSupported);
    //     }

    //     // info_now!("CLA = {:?}", &command.class());
    //     info_now!("INS = {:?}, P1 = {:X}, P2 = {:X}",
    //               &command.instruction(),
    //               command.p1, command.p2,
    //               );
    //     // info_now!("extended = {:?}", command.extended);

    //     // info_now!("INS = {:?}" &command.instruction());
    //     match command.instruction() {
    //         Instruction::GetData => self.get_data(command, reply),
    //         Instruction::PutData => self.put_data(command),
    //         Instruction::Verify => panic!(),//self.old_verify(command),
    //         Instruction::ChangeReferenceData => panic!(),//self.change_reference_data(command),
    //         Instruction::GeneralAuthenticate => self.general_authenticate(command, reply),
    //         Instruction::GenerateAsymmetricKeyPair => self.generate_asymmetric_keypair(command, reply),

    //         Instruction::Unknown(ins) => {

    //             // see if it's a Yubico thing
    //             if let Ok(instruction) = YubicoPivExtension::try_from(ins) {
    //                 self.yubico_piv_extension(command, instruction, reply)
    //             } else {
    //                 Err(Status::FunctionNotSupported)
    //             }
    //         }

    //         _ => Err(Status::FunctionNotSupported),
    //     }
    // }

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
    fn general_authenticate(&mut self, command: &IsoCommand, reply: &mut response::Data) -> Result {

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
            return self.request_for_witness(command, data, reply);
        }

        // step 2 of piv-go/ykAuthenticate
        // https://github.com/go-piv/piv-go/blob/d5ec95eb3bec9c20d60611fb77b7caeed7d886b6/piv/piv.go#L415-L420
        if data.starts_with(&[0x80, 0x08]) {
            data = &data[2..];
            return self.request_for_challenge(command, data, reply);
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

        info_now!("looking for keyreference");
        let key_handle = match self.state.persistent(&mut self.trussed).state.keys.authentication_key {
            Some(key) => key,
            None => return Err(Status::KeyReferenceNotFound),
        };

        let commitment = data; // 32B of data // 150B for ed25519
        let signature = try_syscall!(self.trussed.sign_ed255(key_handle, commitment))
            .map_err(|_error| {
                // NoSuchKey
                debug_now!("{:?}", &_error);
                Status::UnspecifiedNonpersistentExecutionError }
            )?
            .signature;

        piv_types::DynamicAuthenticationTemplate::with_response(&signature)
            .encode_to_heapless_vec(reply)
            // todo: come up with error handling (in this case, shouldn't fail)
            .unwrap();

        Ok(())
    }

    fn request_for_challenge(&mut self, command: &IsoCommand, remaining_data: &[u8], reply: &mut response::Data) -> Result {
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
            debug_now!("{:?}", &our_challenge);
            debug_now!("{:?}", &response);
            return Err(Status::IncorrectDataParameter);
        }

        self.state.runtime.app_security_status.management_verified = true;

        // B) encrypt their challenge
        let (header, challenge) = data.split_at(2);
        if header != &[0x81, 0x08] {
            return Err(Status::IncorrectDataParameter);
        }

        let key = self.state.persistent(&mut self.trussed).state.keys.management_key;

        let encrypted_challenge = syscall!(self.trussed.encrypt_tdes(key, challenge)).ciphertext;

        piv_types::DynamicAuthenticationTemplate::with_response(&encrypted_challenge)
            .encode_to_heapless_vec(reply)
            .unwrap();

        Ok(())
    }

    fn request_for_witness(&mut self, command: &IsoCommand, remaining_data: &[u8], reply: &mut response::Data) -> Result {
        // invariants: parsed data was '7C L1 80 00' + remaining_data

        if command.p1 != 0x03 || command.p2 != 0x9b {
            return Err(Status::IncorrectP1OrP2Parameter);
        }

        if !remaining_data.is_empty() {
            return Err(Status::IncorrectDataParameter);
        }

        let key = self.state.persistent(&mut self.trussed).state.keys.management_key;

        let challenge = syscall!(self.trussed.random_bytes(8)).bytes;
        let command_cache = state::AuthenticateManagement { challenge: challenge[..].try_into().unwrap() };
        self.state.runtime.command_cache = Some(state::CommandCache::AuthenticateManagement(command_cache));

        let encrypted_challenge = syscall!(self.trussed.encrypt_tdes(key, &challenge)).ciphertext;

        piv_types::DynamicAuthenticationTemplate::with_witness(&encrypted_challenge)
            .encode_to_heapless_vec(reply)
            .unwrap();

        Ok(())

    }

    //fn change_reference_data(&mut self, command: &Command) -> Result {
    //    // The way `piv-go` blocks PUK (which it needs to do because Yubikeys only
    //    // allow their Reset if PIN+PUK are blocked) is that it sends "change PUK"
    //    // with random (i.e. incorrect) PUK listed as both old and new PUK.
    //    //
    //    // 00 24 00 81 10
    //    //       32 38 36 34 31 39 30 36 32 38 36 34 31 39 30 36
    //    //
    //    // For now, we don't support PUK, so we can just return "Blocked" directly
    //    // if the key reference in P2 is '81' = PUK

    //    // application PIN
    //    if command.p2 == 0x80 {
    //        let remaining_retries = self.state.persistent(&mut self.trussed).remaining_pin_retries();

    //        if remaining_retries == 0 {
    //            return Err(Status::OperationBlocked);
    //        }

    //        if command.data().len() != 16 {
    //            return Err(Status::IncorrectDataParameter);
    //        }

    //        let (old_pin, new_pin) = command.data().split_at(8);

    //        let old_pin = match state::Pin::try_new(old_pin) {
    //            Ok(pin) => pin,
    //            _ => return Err(Status::IncorrectDataParameter),
    //        };

    //        let new_pin = match state::Pin::try_new(new_pin) {
    //            Ok(pin) => pin,
    //            _ => return Err(Status::IncorrectDataParameter),
    //        };

    //        if !self.state.persistent(&mut self.trussed).verify_pin(&old_pin) {
    //            let remaining = self.state.persistent(&mut self.trussed).increment_consecutive_pin_mismatches(&mut self.trussed);
    //            self.state.runtime.app_security_status.pin_verified = false;
    //            return Err(Status::RemainingRetries(remaining));
    //        }

    //        self.state.persistent(&mut self.trussed).reset_consecutive_pin_mismatches(&mut self.trussed);
    //        self.state.persistent(&mut self.trussed).set_pin(&mut self.trussed, new_pin);
    //        self.state.runtime.app_security_status.pin_verified = true;
    //        return Ok(());
    //    }

    //    // PUK
    //    if command.p2 == 0x81 {
    //        let remaining_retries = self.state.persistent(&mut self.trussed).remaining_puk_retries();

    //        if remaining_retries == 0 {
    //            return Err(Status::OperationBlocked);
    //        }

    //        if command.data().len() != 16 {
    //            return Err(Status::IncorrectDataParameter);
    //        }

    //        let (old_puk, new_puk) = command.data().split_at(8);

    //        let old_puk = match state::Pin::try_new(old_puk) {
    //            Ok(puk) => puk,
    //            _ => return Err(Status::IncorrectDataParameter),
    //        };

    //        let new_puk = match state::Pin::try_new(new_puk) {
    //            Ok(puk) => puk,
    //            _ => return Err(Status::IncorrectDataParameter),
    //        };

    //        if !self.state.persistent(&mut self.trussed).verify_puk(&old_puk) {
    //            let remaining = self.state.persistent(&mut self.trussed).increment_consecutive_puk_mismatches(&mut self.trussed);
    //            self.state.runtime.app_security_status.puk_verified = false;
    //            return Err(Status::RemainingRetries(remaining));
    //        }

    //        self.state.persistent(&mut self.trussed).reset_consecutive_puk_mismatches(&mut self.trussed);
    //        self.state.persistent(&mut self.trussed).set_puk(&mut self.trussed, new_puk);
    //        self.state.runtime.app_security_status.puk_verified = true;
    //        return Ok(());
    //    }


    //    Err(Status::KeyReferenceNotFound)
    //}

    //fn old_verify(&mut self, command: &Command) -> Result {
    //    // we only implement our own PIN, not global Pin, not OCC data, not pairing code
    //    if command.p2 != 0x80 {
    //        return Err(Status::KeyReferenceNotFound);
    //    }

    //    let p1 = command.p1;
    //    if p1 != 0x00 && p1 != 0xFF {
    //        return Err(Status::IncorrectP1OrP2Parameter);
    //    }

    //    // all above failures shall not change security status or retry counter

    //    // 1) If p1 is FF, "log out" of PIN
    //    if p1 == 0xFF {
    //        if command.data().len() != 0 {
    //            return Err(Status::IncorrectDataParameter);
    //        } else {
    //            self.state.runtime.app_security_status.pin_verified = false;
    //            return Ok(());
    //        }
    //    }

    //    // 2) Get retries (or whether verification is even needed) by passing no data
    //    if p1 == 0x00 && command.data().len() == 0 {
    //        if self.state.runtime.app_security_status.pin_verified {
    //            return Ok(());
    //        } else {
    //            let retries = self.state.persistent(&mut self.trussed).remaining_pin_retries();
    //            return Err(Status::RemainingRetries(retries));
    //        }
    //    }

    //    // if malformed PIN is sent, no security implication
    //    if command.data().len() != 8 {
    //        return Err(Status::IncorrectDataParameter);
    //    }

    //    let sent_pin = match state::Pin::try_new(&command.data()) {
    //        Ok(pin) => pin,
    //        _ => return Err(Status::IncorrectDataParameter),
    //    };

    //    // 3) Verify le PIN!
    //    let remaining_retries = self.state.persistent(&mut self.trussed).remaining_pin_retries();
    //    if remaining_retries == 0 {
    //        return Err(Status::OperationBlocked);
    //    }

    //    if self.state.persistent(&mut self.trussed).verify_pin(&sent_pin) {
    //        self.state.persistent(&mut self.trussed).reset_consecutive_pin_mismatches(&mut self.trussed);
    //        self.state.runtime.app_security_status.pin_verified = true;
    //        Ok(())

    //    } else {
    //        let remaining = self.state.persistent(&mut self.trussed).increment_consecutive_pin_mismatches(&mut self.trussed);
    //        self.state.runtime.app_security_status.pin_verified = false;
    //        Err(Status::RemainingRetries(remaining))
    //    }
    //}

    fn generate_asymmetric_keypair(&mut self, command: &IsoCommand, reply: &mut response::Data) -> Result {
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
        }).map_err(|_e| {
                info_now!("error parsing GenerateAsymmetricKeypair: {:?}", &_e);
                Status::IncorrectDataParameter
        })?;

        // if mechanism != &[0x11] {
        // HA! patch in Ed255
        if mechanism != &[0x22] {
            return Err(Status::InstructionNotSupportedOrInvalid);
        }

        // ble policy

        if let Some(key) = self.state.persistent(&mut self.trussed).state.keys.authentication_key {
            syscall!(self.trussed.delete(key));
        }

        // let key = syscall!(self.trussed.generate_p256_private_key(
        // let key = syscall!(self.trussed.generate_p256_private_key(
        let key = syscall!(self.trussed.generate_ed255_private_key(
            trussed::types::Location::Internal,
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

        self.state.persistent(&mut self.trussed).state.keys.authentication_key = Some(key);
        self.state.persistent(&mut self.trussed).save();

        // let public_key = syscall!(self.trussed.derive_p256_public_key(
        let public_key = syscall!(self.trussed.derive_ed255_public_key(
            key,
            trussed::types::Location::Volatile,
        )).key;

        let serialized_public_key = syscall!(self.trussed.serialize_key(
            // trussed::types::Mechanism::P256,
            trussed::types::Mechanism::Ed255,
            public_key.clone(),
            trussed::types::KeySerialization::Raw,
        )).serialized_key;

        // info_now!("supposed SEC1 pubkey, len {}: {:X?}", serialized_public_key.len(), &serialized_public_key);

        // P256 SEC1 has 65 bytes, Ed255 pubkeys have 32
        // let l2 = 65;
        let l2 = 32;
        let l1 = l2 + 2;

        reply.extend_from_slice(&[0x7f, 0x49, l1, 0x86, l2]).unwrap();
        reply.extend_from_slice(&serialized_public_key).unwrap();

        Ok(())
    }

    fn put_data(&mut self, command: &IsoCommand) -> Result {
        info_now!("PutData");
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
        }).map_err(|_e| {
                info_now!("error parsing PutData: {:?}", &_e);
                Status::IncorrectDataParameter
        })?;

        // info_now!("PutData in {:?}: {:?}", data_object, data);

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

            try_syscall!(self.trussed.write_file(
                trussed::types::Location::Internal,
                trussed::types::PathBuf::from(b"printed-information"),
                trussed::types::Message::try_from_slice(data).unwrap(),
                None,
            )).map_err(|_| Status::NotEnoughMemory)?;

            return Ok(());
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

            try_syscall!(self.trussed.write_file(
                trussed::types::Location::Internal,
                trussed::types::PathBuf::from(b"authentication-key.x5c"),
                trussed::types::Message::try_from_slice(data).unwrap(),
                None,
            )).map_err(|_| Status::NotEnoughMemory)?;

            return Ok(Default::default());
        }

        Err(Status::IncorrectDataParameter)
    }

    fn get_data(&mut self, command: &IsoCommand, reply: &mut response::Data) -> Result {
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
        info_now!("looking up {:?}", data);

        // TODO: check security status, else return Status::SecurityStatusNotSatisfied

        // Table 3, Part 1, SP 800-73-4
        // https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-73-4.pdf#page=30
        match data {
            DataObjects::DiscoveryObject => {
                // Err(Status::InstructionNotSupportedOrInvalid)
                let data = response::Data::try_from_slice(DISCOVERY_OBJECT).unwrap();
                reply.extend_from_slice(&data).ok();
                // todo!("discovery object"),
            }

            DataObjects::BiometricInformationTemplate => {
                return Err(Status::InstructionNotSupportedOrInvalid)
                // todo!("biometric information template"),
            }

            // '5FC1 02' (351B)
            DataObjects::CardHolderUniqueIdentifier => {
                piv_types::CardHolderUniqueIdentifier::default()
                    .encode_to_heapless_vec(reply)
                    .unwrap();
            }

            // '5FC1 05' (351B)
            DataObjects::X509CertificateForPivAuthentication => {
                // return Err(Status::NotFound);

                // info_now!("loading 9a cert");
                // it seems like fetching this certificate is the way Filo's agent decides
                // whether the key is "already setup":
                // https://github.com/FiloSottile/yubikey-agent/blob/8781bc0082db5d35712a2244e3ab3086f415dd59/setup.go#L69-L70
                let data = try_syscall!(self.trussed.read_file(
                    trussed::types::Location::Internal,
                    trussed::types::PathBuf::from(b"authentication-key.x5c"),
                )).map_err(|_| {
                    // info_now!("error loading: {:?}", &e);
                    Status::NotFound
                } )?.data;

                // todo: cleanup
                let tag = flexiber::Tag::application(0x13); // 0x53
                flexiber::TaggedSlice::from(tag, &data)
                    .unwrap()
                    .encode_to_heapless_vec(reply)
                    .unwrap();
            }

            // '5F FF01' (754B)
            YubicoObjects::AttestationCertificate => {
                let data = response::Data::try_from_slice(YUBICO_ATTESTATION_CERTIFICATE).unwrap();
                reply.extend_from_slice(&data).ok();
            }

            _ => return Err(Status::NotFound),
        }
        Ok(())
    }

    fn yubico_piv_extension(&mut self, command: &IsoCommand, instruction: YubicoPivExtension, reply: &mut response::Data) -> Result {
        info_now!("yubico extension: {:?}", &instruction);
        match instruction {
            YubicoPivExtension::GetSerial => {
                // make up a 4-byte serial
                let data = response::Data::try_from_slice(
                    &[0x00, 0x52, 0xf7, 0x43]).unwrap();
                reply.extend_from_slice(&data).ok();
            }

            YubicoPivExtension::GetVersion => {
                // make up a version, be >= 5.0.0
                let data = response::Data::try_from_slice(
                    &[0x06, 0x06, 0x06]).unwrap();
                reply.extend_from_slice(&data).ok();
            }

            YubicoPivExtension::Attest => {
                if command.p2 != 0x00 {
                    return Err(Status::IncorrectP1OrP2Parameter);
                }

                let slot = command.p1;

                if slot == 0x9a {
                    let data = response::Data::try_from_slice(YUBICO_ATTESTATION_CERTIFICATE_FOR_9A).unwrap();
                    reply.extend_from_slice(&data).ok();
                } else {

                    return Err(Status::FunctionNotSupported)
                }
            }

            YubicoPivExtension::Reset => {
                if command.p1 != 0x00 || command.p2 != 0x00 {
                    return Err(Status::IncorrectP1OrP2Parameter);
                }

                // TODO: find out what all needs resetting :)
                self.state.persistent(&mut self.trussed).reset_pin();
                self.state.persistent(&mut self.trussed).reset_puk();
                self.state.persistent(&mut self.trussed).reset_management_key();
                self.state.runtime.app_security_status.pin_verified = false;
                self.state.runtime.app_security_status.puk_verified = false;
                self.state.runtime.app_security_status.management_verified = false;

                try_syscall!(self.trussed.remove_file(
                    trussed::types::Location::Internal,
                    trussed::types::PathBuf::from(b"printed-information"),
                )).ok();

                try_syscall!(self.trussed.remove_file(
                    trussed::types::Location::Internal,
                    trussed::types::PathBuf::from(b"authentication-key.x5c"),
                )).ok();

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
                self.state.persistent(&mut self.trussed).set_management_key(&new_management_key);

            }

            _ => return Err(Status::FunctionNotSupported),
        }
        Ok(())
    }

}


#[cfg(feature = "apdu-dispatch")]
impl<T> apdu_dispatch::app::Aid for Authenticator<T> {

    fn aid(&self) -> &'static [u8] {
        &constants::PIV_AID
    }

    fn right_truncated_length(&self) -> usize {
        11
    }
}


#[cfg(feature = "apdu-dispatch")]
impl<T> apdu_dispatch::app::App<apdu_dispatch::command::Size, apdu_dispatch::response::Size> for Authenticator<T>
where
    T: client::Client + client::Ed255 + client::Tdes
{
    fn select(&mut self, apdu: &IsoCommand, reply: &mut response::Data) -> Result {
        self.select(apdu, reply)
    }

    fn deselect(&mut self) { self.deselect() }

    fn call(&mut self, _: iso7816::Interface, apdu: &IsoCommand, reply: &mut response::Data) -> Result {
        self.respond(apdu, reply)
    }
}
