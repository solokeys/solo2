#![cfg_attr(not(test), no_std)]

pub mod constants;
pub mod state;

use core::cell::RefCell;
use core::convert::TryFrom;

use cortex_m_semihosting::{dbg, hprintln};
use trussed::{
    Client as Trussed,
    pipe::Syscall,
};
use heapless_bytes::consts;
use interchange::Responder;

use iso7816::{
    Command, Instruction, Response, Status,
    response::{
        Result as ResponseResult,
        Data as ResponseData,
    },
};

use usbd_ccid::{
    constants::*,
    der::Der,
    types::{
        ApduInterchange,
        MessageBuffer,
        packet,
    },
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

    fn try_handle(&mut self, request: &Command) -> ResponseResult {

        // TEMP
        dbg!(self.state.persistent(&mut self.trussed).timestamp(&mut self.trussed));

        // handle CLA
        // - command chaining not supported
        // - secure messaging not supported
        // - only channel zero supported
        // - ensure INS known to us

        let class = request.class();

        if !class.chain().last_or_only() {
            return Err(Status::CommandChainingNotSupported);
        }

        if !class.secure_messaging().none() {
            return Err(Status::SecureMessagingNotSupported);
        }

        if class.channel() != Some(0) {
            return Err(Status::LogicalChannelNotSupported);
        }

        match request.instruction() {
            Instruction::Select => self.select(request),
            _ => Err(Status::FunctionNotSupported),
        }
    }

    fn select(&mut self, request: &Command) -> ResponseResult {
        use state::Aid;

        if request.data().starts_with(state::PivAid::rid()) {
            hprintln!("got PIV!").ok();

            if request.p1 != 0x04 || request.p2 != 0x00 {
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
            hprintln!("reponse data: {:?}", &response_data).ok();
            Ok(response_data)
        } else {
            hprintln!("got not PIV: {:?}", &request.data()).ok();
            Err(Status::NotFound)
        }
    }
}

        //// handle INS
        //// - do what we can
        //// - else unsupported instruction
        //let (cla, ins, p1, p2) = (request.cla, request.ins, request.p1, request.p2);

        //match (cla, ins, p1, p2) {
        //    // `piv-tool -n` sends SELECT for 'A0 00 00 00 01 01', with Le = 0 (?!)
        //    // we need to handle this one
        //    SELECT => {
        //        // pub const PIV_AID: [u8; 11]
        //        //     = [0xa0, 0x00, 0x00, 0x03, 0x08, // 0x00, 0x00, 0x10, 0x00, 0x01, 0x00];
        //        //
        //        // 05808693 APDU: 00 A4 04 00 05 A0 00 00 03 08
        //        hprintln!("got SELECT").ok();
        //        let is_nist_rid = &request.data == NIST_RID;
        //        let is_piv = &request.data == &PIV_AID;
        //        let is_trunc_piv = &request.data == &PIV_TRUNCATED_AID;
        //        let is_pivish = is_piv || is_trunc_piv || is_nist_rid;
        //        let is_yubico = &request.data == &YUBICO_OTP_AID;

        //        if is_pivish {
        //            hprintln!("for PIV").ok();
        //            let response = select_piv();
        //            self.interchange.respond(response).unwrap();
        //            hprintln!("interchange state: {:?}", &self.interchange.state())
        //                .ok();
        //            // todo!("put back in queue");
        //        // } else if is_yubico {
        //        //     hprintln!("for Yubico").ok();
        //        //     command.clear();
        //        //     command.extend_from_slice(&[0x04, 0x03, 0x04, 0x01, 0x05, 0x00, 0x05, 0x0F, 0x00, 0x00]);
        //        //     command.extend_from_slice(OK);
        //        } else {
        //            panic!("unknown AID {:?}", &request.data);
        //        }
        //    }
        //    _ => todo!(),
        //}

fn write_apdu(sw1: u8, sw2: u8, data: &[u8], buffer: &mut MessageBuffer) {
    let l = data.len();
    buffer.clear();

    buffer.extend_from_slice(data).unwrap();
    buffer.push(sw1).unwrap();
    buffer.push(sw2).unwrap();
}

//pub fn fake_piv(command: &mut MessageBuffer) {
//    let apdu = match apdu::Apdu::try_from(command.as_ref()) {
//        Ok(apdu) => apdu,
//        Err(_) => {
//            invalid_apdu(command);
//            return;
//        }
//    };
//    if apdu.ins() != packet::CommandType::GetSlotStatus as u8 {
//        hprintln!("{}, {}", packet::CommandType::GetSlotStatus as u8, apdu.ins()).ok();
//        hprintln!(":: {:?}", &apdu).ok();
//    }

//    let (cla, ins, p1, p2, le) = (*&apdu.cla(), apdu.ins(), apdu.p1(), apdu.p2(), apdu.le());

//    // match (cla, ins, p1, p2, le) {
//    match (cla, ins, p1, p2) {
//        // `piv-tool -n` sends SELECT for 'A0 00 00 00 01 01', with Le = 0 (?!)
//        // we need to handle this one
//        SELECT => {
//            // pub const PIV_AID: [u8; 11]
//            //     = [0xa0, 0x00, 0x00, 0x03, 0x08, // 0x00, 0x00, 0x10, 0x00, 0x01, 0x00];
//            //
//            // 05808693 APDU: 00 A4 04 00 05 A0 00 00 03 08
//            hprintln!("got SELECT").ok();
//            let is_nist_rid = apdu.data() == NIST_RID;
//            let is_piv = apdu.data() == &PIV_AID;
//            let is_trunc_piv = apdu.data() == &PIV_TRUNCATED_AID;
//            let is_pivish = is_piv || is_trunc_piv || is_nist_rid;
//            let is_yubico = apdu.data() == YUBICO_OTP_AID;

//            if is_pivish {
//                hprintln!("for PIV").ok();
//                select(command);
//            } else if is_yubico {
//                hprintln!("for Yubico").ok();
//                command.clear();
//                command.extend_from_slice(&[0x04, 0x03, 0x04, 0x01, 0x05, 0x00, 0x05, 0x0F, 0x00, 0x00]);
//                command.extend_from_slice(OK);
//            } else {
//                panic!("unknown AID {:?}", &apdu.data());
//            }
//        }

//        // https://git.io/JfWaX
//        // YKPIV_OBJ_AUTHENTICATION 0x5fc105 /* cert for 9a key */
//        // YKPIV_OBJ_ATTESTATION 0x5fff01 <-- custom Yubico thing
//        GET_DATA => {
//            todo!();
//        }

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

//        // getVersion in Yubico's AID
//        (0x00, 0xfd, 0x00, 0x00) => {
//            command.clear();
//            // command.extend_from_slice(&[0x04, 0x03, 0x04]);
//            command.extend_from_slice(&[0x06, 0x06, 0x06]);
//            command.extend(OK);
//        }
//        // getSerial in Yubico's AID
//        (0x00, 0x01, 0x10, 0x00) => {
//            command.clear();
//            // make one up :)
//            command.extend_from_slice(&[0x00, 0x52, 0xf7, 0x43]);
//            command.extend(OK);
//        }
//        // getSerial in Yubico's AID (alternate version)
//        (0x00, 0xf8, 0x00, 0x00) => {
//            command.clear();
//            // make one up :)
//            command.extend_from_slice(&[0x00, 0x52, 0xf7, 0x43]);
//            command.extend(OK);
//        }
//        // Yubico getAttestation command (attn for, here,
//        (0x00, 0xf9, 0x9a, 0x00) => {
//            todo!();
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

fn invalid_apdu(command: &mut MessageBuffer) {
    command.clear();
    // figure out what the correct error status words are
    command.extend_from_slice(&[0x6a, 0x82]);

}

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

