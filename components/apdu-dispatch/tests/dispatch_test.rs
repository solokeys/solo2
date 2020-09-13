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
use iso7816::{
    Command,
    Status,
};
use interchange::Interchange;

use heapless::ByteBuf;

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

impl Applet for TestApp1 {

    fn select(&mut self, _apdu: Command) -> AppletResult {
        Ok(Default::default())
    }

    fn deselect(&mut self) {
    }

    fn call (&mut self, apdu: Command) -> AppletResult {
        println!("TestApp1::call");
        match apdu.instruction().into() {
            0x10 => {
                let mut buf = ByteBuf::new();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.push(0).unwrap();
                buf.extend_from_slice(apdu.data()).unwrap();
                Ok(AppletResponse::Respond(buf))
            }
            _ => 
                Err(Status::InstructionNotSupportedOrInvalid)
        }
    }

    fn poll (&mut self) -> AppletResult {
        panic!("Should not have idle polls here!");
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

impl Applet for TestApp2 {

    fn select(&mut self, _apdu: Command) -> AppletResult {
        Ok(Default::default())
    }

    fn deselect(&mut self) {
    }

    fn call (&mut self, apdu: Command) -> AppletResult {
        println!("TestApp2::call");
        match apdu.instruction().into() {
            0x20 => {
                let mut buf = ByteBuf::new();
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
        panic!("Should not have idle polls here!");
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

impl Applet for PanicApp {

    fn select(&mut self, _apdu: Command) -> AppletResult {
        panic!("Dont call the panic app");
    }

    fn deselect(&mut self) {
        panic!("Dont call the panic app");
    }

    fn call (&mut self, _apdu: Command) -> AppletResult {
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
    let (mut contact_requester, contact_responder) = ContactInterchange::claim(0)
        .expect("could not setup ccid ApduInterchange");

    let (_contactless_requester, contactless_responder) = ContactlessInterchange::claim(0)
        .expect("could not setup iso14443 ApduInterchange");

    let mut apdu_dispatch = apdu_dispatch::dispatch::ApduDispatch::new(contact_responder, contactless_responder);

    let mut app1 = TestApp1{};
    let mut app2 = TestApp2{};
    let mut app3 = PanicApp{};

    // for i in 0..apdu_response_pairs.len() {
        // print!("- "); 
        // dump_hex(apdu_response_pairs[i]);
    // }
    for i in (0..apdu_response_pairs.len()).step_by(2) {
        let raw_req = apdu_response_pairs[i];
        let raw_expected_res = apdu_response_pairs[i + 1];

        let command = Command::try_from(raw_req).unwrap();
        // let expected_response = Response::Data::from_slice(&raw_res);

        print!("<< "); 
        dump_hex(&raw_req);

        contact_requester.request(command).expect("could not deposit command");

        apdu_dispatch.poll(&mut[&mut app1, &mut app2, &mut app3]);

        let response = contact_requester.take_response().unwrap().into_message();

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
fn test_select_1(){
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
fn test_select_2(){
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
fn test_select_not_found(){
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
fn test_echo_1(){
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
fn test_echo_wrong_instruction(){
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
fn test_echo_2(){
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
fn test_echo_wrong_instruction_2(){
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
fn test_unsolicited_instruction(){
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
fn test_deselect (){
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
fn test_extended_length_echo (){
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



