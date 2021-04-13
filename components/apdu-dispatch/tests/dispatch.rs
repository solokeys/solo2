use apdu_dispatch::applet::{
    Applet,
    Aid,
    Response as AppletResponse,
    Result as AppletResult,
};
use apdu_dispatch::types::{
    ContactlessInterchange,
    ContactInterchange,
};
use apdu_dispatch::dispatch;
use iso7816::{
    Command,
    Status,
    command,
};
use interchange::Interchange;

use heapless_bytes::Bytes;

#[macro_use]
extern crate serial_test;

#[allow(dead_code)]
enum TestInstruction {
    Echo = 0x10,
    Add = 0x11,
    GetData = 0x12,
}

fn dump_hex(data: &[u8]){
    for i in 0 .. data.len() {
        print!("{:02X} ", data[i]);
    }
    println!();
}

pub struct TestApp1 {}

impl Aid for TestApp1 {
    fn aid(&self) -> &'static [u8] {
        &[ 0x0Au8, 1, 0, 0, 1]
    }

    fn right_truncated_length(&self) -> usize {
        5
    }
}

// This app echos to Ins code 0x10
impl Applet for TestApp1 {

    fn select(&mut self, _apdu: &Command) -> AppletResult {
        Ok(Default::default())
    }

    fn deselect(&mut self) {
    }

    fn call (&mut self, _interface_type: dispatch::InterfaceType, apdu: &Command) -> AppletResult {
        println!("TestApp1::call");
        match apdu.instruction().into() {
            0x10 => {
                let mut buf = Bytes::new();
                // Just echo 5x 0's for the request apdu header
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.extend_from_slice(apdu.data()).unwrap();
                Ok(AppletResponse::Respond(buf))
            }
            // For measuring the stack burden of dispatch
            0x15 => {
                let mut buf = Bytes::new();
                let addr = (&buf as *const iso7816::response::Data ) as u32;
                buf.extend_from_slice(&addr.to_be_bytes()).unwrap();
                Ok(AppletResponse::Respond(buf))
            }
            _ => 
                Err(Status::InstructionNotSupportedOrInvalid)
        }
    }

    fn poll (&mut self) -> AppletResult {
        Ok(AppletResponse::Defer)
    }
}

pub struct TestApp2 {}

impl Aid for TestApp2 {
    fn aid(&self) -> &'static [u8] {
        &[ 0x0Au8, 1, 0, 0, 2]
    }

    fn right_truncated_length(&self) -> usize {
        5
    }
}

// This app echos to Ins code 0x20
impl Applet for TestApp2 {

    fn select(&mut self, _apdu: &Command) -> AppletResult {
        Ok(Default::default())
    }

    fn deselect(&mut self) {
    }

    fn call (&mut self, _interface_type: dispatch::InterfaceType, apdu: &Command) -> AppletResult {
        println!("TestApp2::call");
        match apdu.instruction().into() {
            0x20 => {
                let mut buf = Bytes::new();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.extend_from_slice(apdu.data()).unwrap();
                Ok(AppletResponse::Respond(buf))
            },
            _ =>
                Err(Status::InstructionNotSupportedOrInvalid)
        }
    }

    fn poll (&mut self) -> AppletResult {
        panic!("Should not have idle polls for TestApp2!");
    }
}

pub struct PanicApp {}

impl Aid for PanicApp{
    fn aid(&self) -> &'static [u8] {
        &[ 0x0Au8, 1, 0, 0, 3]
    }

    fn right_truncated_length(&self) -> usize {
        5
    }
}

// This app should never get selected
impl Applet for PanicApp {

    fn select(&mut self, _apdu: &Command) -> AppletResult {
        panic!("Dont call the panic app");
    }

    fn deselect(&mut self) {
        panic!("Dont call the panic app");
    }

    fn call (&mut self, _interface_type: dispatch::InterfaceType, _apdu: &Command) -> AppletResult {
        panic!("Dont call the panic app");
    }

    fn poll (&mut self) -> AppletResult {
        panic!("Dont call the panic app");
    }
}

fn run_apdus(
    apdu_response_pairs: &[&[u8]],
){
    assert!(apdu_response_pairs.len() > 0);
    assert!((apdu_response_pairs.len() & 1) == 0);
    unsafe { ContactInterchange::reset_claims() };
    unsafe { ContactlessInterchange::reset_claims() };
    let (mut contact_requester, contact_responder) = ContactInterchange::claim()
        .expect("could not setup ccid ApduInterchange");

    let (_contactless_requester, contactless_responder) = ContactlessInterchange::claim()
        .expect("could not setup iso14443 ApduInterchange");

    let mut apdu_dispatch = apdu_dispatch::dispatch::ApduDispatch::new(contact_responder, contactless_responder);

    let mut app0 = PanicApp{};
    let mut app1 = TestApp1{};
    let mut app2 = PanicApp{};
    let mut app3 = TestApp2{};
    let mut app4 = PanicApp{};

    // for i in 0..apdu_response_pairs.len() {
        // print!("- "); 
        // dump_hex(apdu_response_pairs[i]);
    // }
    for i in (0..apdu_response_pairs.len()).step_by(2) {
        let raw_req = apdu_response_pairs[i];
        let raw_expected_res = apdu_response_pairs[i + 1];

        // let command = Command::try_from(raw_req).unwrap();
        // let expected_response = Response::Data::from_slice(&raw_res);

        print!("<< "); 
        dump_hex(&raw_req);

        contact_requester.request(command::Data::try_from_slice(&raw_req).unwrap())
            .expect("could not deposit command");

        apdu_dispatch.poll(&mut[&mut app0, &mut app1, &mut app2, &mut app3, &mut app4]);

        let response = contact_requester.take_response().unwrap();

        print!(">> "); 
        dump_hex(&response);

        if raw_expected_res != response.as_slice()
        {
            print!("expected: "); 
            dump_hex(&raw_expected_res);
            print!("got: "); 
            dump_hex(&response);
            panic!("Expected responses do not match");
        }
    }
}

#[test]
#[serial]
fn malformed_apdus(){
    run_apdus(
        &[
            // Too short
            &[0x00u8,],
            &[0x6F, 0x00],
            // Too short
            &[0x00u8,0x00u8],
            &[0x6F, 0x00],
            // Too short
            &[0x00u8,0x00,0x00,],
            &[0x6F, 0x00],
            // Wrong length
            &[0x00u8,0x00,0x00,0x00,0x10,1,1,1],
            &[0x6F, 0x00],
            // Extra data
            &[0x00u8,0x00,0x00,0x00,0x5,1,1,1,1,1,1,1,1,1,1,1,1,1],
            &[0x6F, 0x00],
            // Invalid CLA
            &[0xFFu8,0x00,0x00,0x00],
            &[0x6F, 0x00],
            // Invalid extended length
            &[0x00u8,0x00,0x00,0x00,0xff,0x00,0x05,1,1,1,1,1],
            &[0x6F, 0x00],
            // sanity check with Valid APDU with extended length
            &[0x00u8,0x00,0x00,0x00,0x00,0x00,0x05,1,1,1,1,1],
            &[0x6A, 0x82],
        ]
    )
}


#[test]
#[serial]
fn select_1(){
    run_apdus(
        &[
            // Select
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            // Ok
            &[0x90, 0x00],
        ]
    )
}

#[test]
#[serial]
fn select_2(){
    run_apdus(
        &[
            // Select
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x02],
            // Ok
            &[0x90, 0x00],
        ]
    )
}

#[test]
#[serial]
fn select_not_found(){
    run_apdus(
        &[
            // Select
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x01, 0x00],
            // Not found
            &[0x6A, 0x82],
        ]
    )
}

#[test]
#[serial]
fn echo_1(){
    run_apdus(
        &[
            // Select
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            // Ok
            &[0x90, 0x00],

            // Echo
            &[0x00u8, 0x10, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            // Echo + Ok
            &[0x00u8, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x90, 0x00],
        ]
    )
}

#[test]
#[serial]
fn echo_with_cla_bits_set(){
    run_apdus(
        &[
            // Select
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            // Ok
            &[0x90, 0x00],

            // Echo
            &[0x80u8, 0x10, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            // Echo + Ok
            &[0x00u8, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x90, 0x00],
        ]
    )
}

#[test]
#[serial]
fn echo_wrong_instruction(){
    run_apdus(
        &[
            // Select
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            // Ok
            &[0x90, 0x00],

            // Echo
            &[0x00u8, 0x20, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            // Wrong Ins
            &[0x6d, 0x00],
        ]
    )
}

#[test]
#[serial]
fn echo_2(){
    run_apdus(
        &[
            // Select
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x02],
            // Ok
            &[0x90, 0x00],

            // Echo
            &[0x00u8, 0x20, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            // Echo + Ok
            &[0x00u8, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x90, 0x00],
        ]
    )
}

#[test]
#[serial]
fn echo_wrong_instruction_2(){
    run_apdus(
        &[
            // Select
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x02],
            // Ok
            &[0x90, 0x00],

            // Echo
            &[0x00u8, 0x10, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            // Wrong Ins
            &[0x6d, 0x00],
        ]
    )
}

#[test]
#[serial]
fn unsolicited_instruction(){
    run_apdus(
        &[
            // Echo
            &[0x00u8, 0x10, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            // Not found
            &[0x6a, 0x82],
        ]
    )
}

#[test]
#[serial]
fn deselect (){
    run_apdus(
        &[
            // Select 1
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            &[0x90, 0x00],

            // Echo 1
            &[0x00u8, 0x10, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            &[0x00u8, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x90, 0x00],

            // Select 2
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x02],
            &[0x90, 0x00],

            // Echo 1
            &[0x00u8, 0x10, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            &[0x6d, 0x00],
        ]
    )
}

#[test]
#[serial]
fn extended_length_echo (){
    run_apdus(
        &[
            // Select 1
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            &[0x90, 0x00],

            // To be echo'd
            &[0x00u8, 0x10, 0x00, 0x00, 0x00, 0x01, 0x23,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,
            ],
            // echo  Success
            &[0x00, 0x00, 0x00, 0x00, 0x00,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                1,1,1,1,1,1,1,1,1,1,1,
                0x90,00
            ]
        ]
    )
}

#[test]
#[serial]
fn chained_apdu_1 (){
    run_apdus(
        &[
            // Select 1
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            &[0x90, 0x00u8],

            // Set chaining bit
            &[0x10u8, 0x20, 0x00, 0x00, 0xFF,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
            ],
            &[ 0x90,00 ],

            // Set chaining bit
            &[0x10u8, 0x20, 0x00, 0x00, 0xFF,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
            ],
            &[ 0x90,00 ],

            // Send last command
            &[0x00u8, 0x10, 0x00, 0x00, 0x20,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
            ],
            // Expect 0xff + 0xff + 0x20 + 5 == 547 bytes back
            // Echo chunk + remaining
            &[
                /*      1             8               16              24              32 */
                /* 1 */ 0,0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                0x61,00     // Still 291 bytes left
            ],

            // Get Response
            &[0x00u8, 0xC0, 0x00, 0x00],
            // Echo chunk + remaining
            &[
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                0x61,0x23     // Still 35 bytes left
            ],

            // Get Response
            &[0x00u8, 0xC0, 0x00, 0x00],
            // Echo chunk + success
            &[
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,
                0x90,0x00
            ],

            // Get Response
            &[0x00u8, 0xC0, 0x00, 0x00],
            // Error
            &[ 0x6F,0x00 ],
        ]
    )
}


#[test]
#[serial]
fn multiple_chained_apdu_1 (){
    run_apdus(
        &[
            // Select 1
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            &[0x90, 0x00u8],

            // Set chaining bit
            &[0x10u8, 0x20, 0x00, 0x00, 0xFF,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
            ],
            &[ 0x90,00 ],


            // Send last command
            &[0x00u8, 0x10, 0x00, 0x00, 0x20,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
            ],
            // Expect 0xff + 0xff + 0x20 + 5 == 292 bytes back
            // Data + remaining bytes
            &[
                /*      1             8               16              24              32 */
                /* 1 */ 0,0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                0x61,0x24     // Still 36 bytes left
            ],

            // Get Response
            &[0x00u8, 0xC0, 0x00, 0x00],
            // Echo chunk + success
            &[
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,
                0x90,0x00
            ],
            
            // Check short commands still work
            // Echo 1
            &[0x00u8, 0x10, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            &[0x00u8, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x90, 0x00],

            // Echo 2
            &[0x00u8, 0x20, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            &[0x6d, 0x00],

            // Check chaining command still works
            // Set chaining bit
            &[0x10u8, 0x20, 0x00, 0x00, 0xFF,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
            ],
            &[ 0x90,00 ],


            // Send last command
            &[0x00u8, 0x10, 0x00, 0x00, 0x20,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
            ],
            // Expect 0xff + 0xff + 0x20 + 5 == 292 bytes back
            // Data + remaining bytes
            &[
                /*      1             8               16              24              32 */
                /* 1 */ 0,0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                0x61,0x24     // Still 36 bytes left
            ],

            // Get Response
            &[0x00u8, 0xC0, 0x00, 0x00],
            // Echo chunk + success
            &[
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,
                0x90,0x00
            ],
        ]
    )
}


#[test]
#[serial]
fn multiple_chained_apdu_interruption (){
    run_apdus(
        &[
            // Select 1
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            &[0x90, 0x00u8],

            // Set chaining bit
            &[0x10u8, 0x20, 0x00, 0x00, 0xFF,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
            ],
            &[ 0x90,00 ],


            // Send last command
            &[0x00u8, 0x10, 0x00, 0x00, 0x20,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
            ],
            // Expect 0xff + 0xff + 0x20 + 5 == 292 bytes back
            // Data + remaining bytes
            &[
                /*      1             8               16              24              32 */
                /* 1 */ 0,0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                0x61,0x24     // Still 36 bytes left
            ],

            // Just ignore those 36 bytes and do something different
            // Echo 1
            &[0x00u8, 0x10, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05],
            &[0x00u8, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x90, 0x00],

            // GetResponse no longer works
            &[0x00u8, 0xC0, 0x00, 0x00],
            &[ 0x6F,0x00 ],

            // Check that new chaining transaction works
            &[0x10u8, 0x20, 0x00, 0x00, 0xFF,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
            ],
            &[ 0x90,00 ],


            // Send last command
            &[0x00u8, 0x10, 0x00, 0x00, 0x20,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
            ],
            // Expect 0xff + 0xff + 0x20 + 5 == 292 bytes back
            // Data + remaining bytes
            &[
                /*      1             8               16              24              32 */
                /* 1 */ 0,0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
                0x61,0x24     // Still 36 bytes left
            ],

        ]
    )
}

#[test]
#[serial]
fn chaining_with_unknown_class_range(){
    run_apdus(
        &[
            // Select 1
            &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
            &[0x90, 0x00u8],

            // Set chaining bit + upper range bit
            &[0x90u8, 0x20, 0x00, 0x00, 0xFF,
                /*      1             8               16              24              32 */
                /* 1 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 2 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 3 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 4 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 5 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 6 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 7 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, 
                /* 8 */ 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
            ],
            &[ 0x90,00 ],
        ]
    )
}



#[test]
#[serial]
fn check_stack_burden(){

    unsafe { ContactInterchange::reset_claims() };
    unsafe { ContactlessInterchange::reset_claims() };

    let (mut contact_requester, contact_responder) = ContactInterchange::claim()
        .expect("could not setup ccid ApduInterchange");

    let (_contactless_requester, contactless_responder) = ContactlessInterchange::claim()
        .expect("could not setup iso14443 ApduInterchange");

    let mut apdu_dispatch = apdu_dispatch::dispatch::ApduDispatch::new(contact_responder, contactless_responder);

    let mut app1 = TestApp1{};

    contact_requester.request(command::Data::try_from_slice(
        &[0x00u8, 0xA4, 0x04, 0x00, 0x05, 0x0A, 0x01, 0x00, 0x00, 0x01],
    ).unwrap()).expect("could not deposit command");

    apdu_dispatch.poll(&mut[&mut app1]);

    let response = contact_requester.take_response().unwrap();

    print!(">> "); 
    dump_hex(&response);

    contact_requester.request(command::Data::try_from_slice(
        &[0x00u8, 0x15, 0x00, 0x00],
    ).unwrap()).expect("could not deposit command");

    apdu_dispatch.poll(&mut[&mut app1]);

    let response = contact_requester.take_response().unwrap();

    print!(">> "); 
    dump_hex(&response);

    let payload: [u8; 4] = [response[0], response[1], response[2], response[3], ];
    let min_stack = u32::from_be_bytes(payload);
    let max_stack = (&response as *const iso7816::response::Data) as u32;

    println!("Burden: {} bytes", max_stack - min_stack);

    // assert!(false);

}