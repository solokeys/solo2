#[cfg(test)]
use nb;

#[cfg(test)]
use crate::{
    Applet,
    ApduManager,
    AppletResponse,
    SourceError,
    Error,
    Aid,
    types,
};

#[cfg(test)]
use iso7816::{
    Command,
    Response,
    Instruction,
    Status,
};

#[cfg(test)]
use interchange::{Responder, Requester};
#[cfg(test)]
use heapless::ByteBuf;

#[cfg(test)]
/// Source that get's pre-configured with the APDUs to send, and responses to expect to get back.
struct DummySource<'a> {
    i: usize,
    to_send_recv: &'a[&'a[u8]],
    contact: Requester<types::ContactInterchange>,
    contactless: Requester<types::ContactlessInterchange>,
}

#[cfg(test)]
impl<'a> DummySource<'a>{
    pub fn new(to_send_recv: &'a[&'a[u8]]) ->
        ( DummySource<'a>, Responder<types::ContactInterchange>, Responder<types::ContactlessInterchange> ){

        let (contact_requester, contact_responder) = types::ContactInterchange::claim(0);
        let (contactless_requester, contactless_responder) = types::ContactlessInterchange::claim(0);
        if (to_send_recv.len() & 1) == 1 {
            panic!("Need to have even number of APDUs in list!");
        }

        (
            DummySource{
                i:0,
                to_send_recv: to_send_recv,
                contact: contact_requester,
                contactless: contactless_requester,
            },
            contact_responder,
            contactless_responder
        )
    }
}

#[cfg(test)]
impl<'a> DummySource<'a> {

    fn exchanges_num (&self,) -> usize {
        self.to_send_recv.len() / 2
    }

    // to the applet
    fn read_apdu(&mut self) -> Result<(), Status>{
        if self.i < self.to_send_recv.len() {
            let apdu = self.to_send_recv[self.i];
            self.i += 1;
            for i in 0 .. apdu.len() {
                buffer[i] = apdu[i];
            }
            std::println!(">>");
            for i in 0 .. apdu.len() { std::print!(" {:02X}", buffer[i]); }
            std::println!("");

            match Command::try_from(apdu) {
                Ok(command) => {
                    self.contactless.request(command).expect("could not deposit command");
                    Ok(())
                },
                Err(_error) => {
                    std::println!("apdu bad {}", error);
                    Err(Status::UnspecifiedCheckingError)
                }
            }

        }
        else {
            panic!("Not enough input!")
        }
    }

    // from the applet
    fn send_apdu(&mut self) -> Result<(), SourceError> {
        let ref_apdu = self.to_send_recv[self.i];
        if let Some(response) = self.contactless.take_response() {
            let buffer = response.into_message();

            self.i += 1;

            std::println!(">>");
            for i in 0 .. buffer.len() { std::print!(" {:02X}", buffer[i]); }
            std::println!("");

            std::println!("ref vs buffer: {} vs {}", ref_apdu.len(), buffer.len());
            assert!(ref_apdu.len() == buffer.len());
            for i in 0 .. buffer.len() {
                assert!( ref_apdu[i] == buffer[i] );
            }
            Ok(())

        } else {
            panic!("No response!");
        }
    }
}

#[cfg(test)]
struct AppletTest1 {
}
#[cfg(test)]
struct AppletTest2 {
}
#[cfg(test)]
struct AppletEchoPlusOne {
}

#[cfg(test)]
impl Aid for AppletTest1{
    fn aid(&self) -> &'static [u8] {
        &[ 0x0Au8, 1, 0, 0,
            0,0,0,0, 0,0,0,0, 0,0,0,0]
    }

    fn right_truncated_length(&self) -> usize {
        16
    }
}



#[cfg(test)]
impl Applet for AppletTest1 {

    fn select(&mut self, _apdu: Command) -> Result<u16, crate::traits::Error> {
        Ok(Default::default())
    }

    fn deselect(&mut self) -> Result<(), crate::traits::Error> {
        Ok(())
    }

    fn send_recv(&mut self, apdu: Command) -> Result<u16, crate::traits::Error> {
        match apdu.ins {
            0x11 => {
                Ok(AppletResponse::Respond(ByteBuf::from_slice(
                    &[(apdu.p1 + apdu.p2) as u8]
                ).unwrap()))
            }
            0x12 => {
                let buf = ByteBuf::new();
                // Send 150 byte of data
                for i in 0 .. 150 {
                    buf.push(i);
                }
                Ok(AppletResponse::Respond(buf))
            }
            _ => {
                Err(Error::SwInsNotSupported)
            }
        }
    }
}

#[cfg(test)]
impl Aid for AppletTest2{
    fn aid(&self) -> &'static [u8] {
        &[ 0x0Au8, 2, 0, 0,
            0,0,0,0, 0,0,0,0, 0,0,0,0]
    }

    fn right_truncated_length(&self) -> usize {
        16
    }
}


#[cfg(test)]
impl Applet for AppletTest2 {

    // fn aid(&self) -> &AidBuffer { & }

    fn select(&mut self, _apdu: Command) -> Result<u16, crate::traits::Error> {
        Ok(Default::default())
    }

    fn deselect(&mut self) -> Result<(), crate::traits::Error> {
        Ok(())
    }

    fn send_recv(&mut self, apdu: Command) -> Result<u16, crate::traits::Error> {
        match apdu.ins {
            0x22 => {
                Ok(AppletResponse::Respond(ByteBuf::from_slice(
                    &[((apdu.p1 as u32) * (apdu.p2 as u32)) as u8]
                ).unwrap()))
            }
            _ => {
                Err(Error::SwInsNotSupported)
            }
        }
    }
}

#[cfg(test)]
impl Aid for AppletEchoPlusOne {
    fn aid(&self) -> &'static [u8] {
        &[ 0x0Au8, 3, 0, 0,
            0,0,0,0, 0,0,0,0, 0,0,0,0]
    }

    fn right_truncated_length(&self) -> usize {
        16
    }
}



#[cfg(test)]
impl Applet for AppletEchoPlusOne {

    fn select(&mut self, apdu: Command) -> Result<u16, crate::traits::Error> {
        // Just echo 5 bytes (+1) on select

        let buf = ByteBuf::new();
        for i in 0 .. core::cmp::min(apdu.data().len(), 5) {
            buf.push(apdu.data()[i] + 1);
        }

        Ok(AppletResponse::Respond(buf))
    }

    fn deselect(&mut self) -> Result<(), crate::traits::Error> {
        Ok(())
    }

    fn send_recv(&mut self, apdu: Command) -> Result<u16, crate::traits::Error> {
        let buf = ByteBuf::new();
        for i in 0 .. apdu.data().len() {
            buf.push(apdu.data()[i] + 1);
        }
        Ok(AppletResponse::Respond(buf))
    }
}

#[cfg(test)]
macro_rules! assert_exchanges{
    ($manager:expr, $source:expr, $applets:expr) => {
        for i in 0 .. $source.exchanges_num() {
            $source.read_apdu().unwrap();
            $manager.poll($applets);
            $source.send_apdu().unwrap();
        }
    }
}


#[test]
fn test_adpu_manager_select_1(){
    let (mut applet1, mut applet2) = (AppletTest1{}, AppletTest2{});

    let (mut source, mut contact, mut contactless) = DummySource::new( &[
        // Select
        &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x01, 0x00, 0x00],
        // Ok
        &[0x90, 0x00],
    ] );

   let mut manager = ApduManager::new(contact, contactless);

   assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2]);
}

// #[test]
// fn test_adpu_manager_select_2(){
//     let (mut applet1, mut applet2) = (AppletTest1{}, AppletTest2{});

//     let mut source = DummySource::new( &[
//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x02, 0x00, 0x00],
//         // Ok
//         &[0x90, 0x00],
//     ] );

//    let mut manager = ApduManager::new();

//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2], 1);
// }



// #[test]
// fn test_adpu_manager_select_fail(){
//     let (mut applet1, mut applet2) = (AppletTest1{}, AppletTest2{});

//     let mut source = DummySource::new( &[
//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0xff, 0xdd, 0xee],
//         // File not found
//         &[0x6A, 0x82],
//     ] );

//    let mut manager = ApduManager::new();
//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2], 1);

// }

// #[test]
// fn test_adpu_manager_applet_1(){
//     let (mut applet1, mut applet2) = (AppletTest1{}, AppletTest2{});

//     let mut source = DummySource::new( &[
//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x01, 0x00, 0x00],
//         // File not found
//         &[0x90, 0x00],

//         // 0x12 + 0x34
//         &[0x00u8, 0x11, 0x12, 0x34, ],
//         // answer + Success
//         &[0x46, 0x90u8, 0x00, ],

//         // 0x02 + 0x02
//         &[0x00u8, 0x11, 0x02, 0x02, ],
//         // answer + Success
//         &[0x04, 0x90u8, 0x00, ],

//         // 0x50 + 0x60
//         &[0x00u8, 0x11, 0x50, 0x60, ],
//         // answer + Success
//         &[0xB0, 0x90u8, 0x00, ],

//     ] );

//    let mut manager = ApduManager::new();

//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2], 4);
// }

// #[test]
// fn test_adpu_manager_applet_2(){
//     let (mut applet1, mut applet2) = (AppletTest1{}, AppletTest2{});

//     let mut source = DummySource::new( &[
//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x02, 0x00, 0x00],
//         // Success
//         &[0x90, 0x00],

//         // 0x12 * 0x34
//         &[0x00u8, 0x22, 0x12, 0x34, ],
//         // answer + Success
//         &[0xa8, 0x90u8, 0x00],
//     ] );

//    let mut manager = ApduManager::new();

//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2], 2);
// }

// #[test]
// fn test_adpu_manager_applet_no_select(){
//     let (mut applet1, mut applet2) = (AppletTest1{}, AppletTest2{});

//     let mut source = DummySource::new( &[
//         // 0x12 * 0x34
//         &[0x00u8, 0x11, 0x12, 0x34, ],

//         // File not found
//         &[0x6A, 0x82],
//     ] );

//    let mut manager = ApduManager::new();

//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2], 1);
// }

// #[test]
// fn test_adpu_manager_applet_bad_ins(){
//     let (mut applet1, mut applet2) = (AppletTest1{}, AppletTest2{});

//     let mut source = DummySource::new( &[
//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x02, 0x00, 0x00],
//         // Success
//         &[0x90, 0x00],

//         // bad ins
//         &[0x00u8, 0xff, 0x12, 0x34, ],

//         // SwInsNotSupported
//         &[0x6Du8, 0x00, ],
//     ] );

//    let mut manager = ApduManager::new();

//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2], 2);
// }

// #[test]
// fn test_adpu_manager_applet_large_read(){
//     let (mut applet1, mut applet2, mut applet3) = (AppletTest1{}, AppletTest2{}, AppletEchoPlusOne{});

//     let mut source = DummySource::new( &[
//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x01, 0x00, 0x00],
//         // Success
//         &[0x90, 0x00,],

//         // Test read of large data
//         &[0x00u8, 0x12, 0x00, 0x00,],
//         // 150 bytes + Success
//         &[
//         0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
//         16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
//         30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43,
//         44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57,
//         58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71,
//         72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85,
//         86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99,
//         100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
//         112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123,
//         124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135,
//         136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147,
//         148, 149,
//         0x90u8, 0x00,
//         ],
//     ] );

//    let mut manager = ApduManager::new();

//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2, &mut applet3], 2);
// }



// #[test]
// fn test_adpu_manager_applet_echo(){
//     let (mut applet1, mut applet2, mut applet3) = (AppletTest1{}, AppletTest2{}, AppletEchoPlusOne{});

//     let mut source = DummySource::new( &[
//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x03, 0x00, 0x00],
//         // 5 byte echo (incremented) + Success
//         &[0x01, 0xA5, 0x05, 0x01, 0x05, 0x90, 0x00,],

//         // To be echo'd
//         &[0x00u8, 0x11, 0x22, 0x33, 0x04, 0x55, 0x66, 0x77, 0x88],
//         // echo (incremented) + Success
//         &[0x01u8, 0x12, 0x23, 0x34, 0x05, 0x56, 0x67, 0x78, 0x89, 0x90u8, 0x00,],
//     ] );

//    let mut manager = ApduManager::new();

//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2, &mut applet3], 2);
// }

// #[test]
// fn test_adpu_manager_switch_applets(){
//     let (mut applet1, mut applet2, mut applet3) = (AppletTest1{}, AppletTest2{}, AppletEchoPlusOne{});

//     let mut source = DummySource::new( &[
//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x03, 0x00, 0x00],
//         // 5 byte echo (incremented) + Success
//         &[0x01, 0xA5, 0x05, 0x01, 0x05, 0x90, 0x00,],

//         // To be echo'd
//         &[0x00u8, 0x11, 0x22, 0x33, 0x04, 0x55, 0x66, 0x77, 0x88],
//         // echo (incremented) + Success
//         &[0x01u8, 0x12, 0x23, 0x34, 0x05, 0x56, 0x67, 0x78, 0x89, 0x90u8, 0x00, ],


//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x01, 0x00, 0x00],
//         // Success
//         &[0x90, 0x00],

//         // add
//         &[0x00u8, 0x11, 0x11, 0x11],
//         // answer + Success
//         &[0x22, 0x90u8, 0x00, ],


//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x02, 0x00, 0x00],
//         // Success
//         &[0x90, 0x00],

//         // mult
//         &[0x00u8, 0x22, 0x08, 0x02],
//         // answer + Success
//         &[0x10, 0x90u8, 0x00],
//     ] );

//    let mut manager = ApduManager::new();

//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2, &mut applet3], 6);
// }


// #[test]
// fn test_adpu_manager_applet_echo_extended_length(){
//     let (mut applet1, mut applet2, mut applet3) = (AppletTest1{}, AppletTest2{}, AppletEchoPlusOne{});

//     let mut source = DummySource::new( &[
//         // Select
//         &[0x00u8, 0xA4, 0x04, 0x00, 0x04, 0x0A, 0x03, 0x00, 0x00],
//         // 5 byte echo (incremented) + Success
//         &[0x01, 0xA5, 0x05, 0x01, 0x05, 0x90, 0x00, ],

//         // To be echo'd
//         &[0x00u8, 0x11, 0x22, 0x33, 0x00, 0x01, 0x23,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//             1,1,1,1,1,1,1,1,1,1,1,
//         ],
//         // echo (incremented) + Success
//         &[0x01u8, 0x12, 0x23, 0x34, 0x01, 0x02, 0x24,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
//             0x90u8, 0x00,
//         ],
//     ] );

//    let mut manager = ApduManager::new();

//    assert_exchanges!(&mut manager, &mut source, &mut[&mut applet1, &mut applet2, &mut applet3], 2);
// }





