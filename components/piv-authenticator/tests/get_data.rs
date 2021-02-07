mod setup;

// use delog::hex_str;
// use iso7816::Status::*;

#[test]
fn get_data() {
    // let cmd = cmd!("00 47 00 9A 0B  AC 09  80 01 11  AA 01 02  AB 01 02");
    // let cmd = cmd!("00 47 00 9A 0B  AC 09  80 01 11  AA 01 02  AB 01 02");

    // let cmd = cmd!("00 f8 00 00");
//     // without PIN, no key generation
    setup::piv(|piv| {
        // ykGetSerial
        // println!("{}", hex_str!(&piv.respond(&cmd!("00 f8 00 00")).unwrap()));
        // panic!();
        piv.respond(&cmd!("00 f8 00 00")).unwrap();
        // assert_eq!([].as_ref(), piv.respond(&cmd!("00 f8 00 00")).unwrap());
        // ykGetVersion
        piv.respond(&cmd!("00 fd 00 00")).unwrap();
    });
}
