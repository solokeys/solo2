mod setup;

// use apdu_dispatch::dispatch::Interface::Contact;
// use apdu_dispatch::app::App as _;
// use hex_literal::hex;
// use iso7816::Command;

// # PutData
// 00 DB 3F FF 23
//    # data object: 5FC109
//    5C 03 5F C1 09
//    # data:
//    53 1C
//       # actual data
//       88 1A 89 18 AA 81 D5 48 A5 EC 26 01 60 BA 06 F6 EC 3B B6 05 00 2E B6 3D 4B 28 7F 86

#[test]
fn put_data() {
    setup::piv(|piv| {

        let _response = piv.respond(&cmd!(
            "00 DB 3F FF 23 5C 03 5F C1 09 53 1C 88 1A 89 18 AA 81 D5 48 A5 EC 26 01 60 BA 06 F6 EC 3B B6 05 00 2E B6 3D 4B 28 7F 86"
        )).unwrap();
    });
}
