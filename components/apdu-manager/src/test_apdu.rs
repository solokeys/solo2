
#[cfg(test)]
use crate::{
    Apdu,
    Error
};

#[test]
fn test_apdu_case_1() {
    let mut apdu_bin: [u8; 4] = [0xaa,0xbb,0x01, 0x02];

    let apdu = Apdu::new_fixed(&mut apdu_bin).ok().unwrap();

    assert!(apdu.case == 1);
    assert!(apdu.cla == 0xaa);
    assert!(apdu.ins == 0xbb);
    assert!(apdu.p1 == 0x01);
    assert!(apdu.p2 == 0x02);

    assert!(apdu.lc == 0);
    assert!(apdu.le == 0);
}

#[test]
fn test_apdu_case_2() {
    let mut apdu_bin: [u8; 5] = [0xaa,0xbb,0x01, 0x02, 5];

    let apdu = Apdu::new_fixed(&mut apdu_bin).ok().unwrap();

    assert!(apdu.case == 2);
    assert!(apdu.cla == 0xaa);
    assert!(apdu.ins == 0xbb);
    assert!(apdu.p1 == 0x01);
    assert!(apdu.p2 == 0x02);

    assert!(apdu.lc == 0);
    assert!(apdu.le == 5);
}


#[test]
fn test_apdu_case_3() {
    let mut apdu_bin: [u8; 5 + 5] = [0xaa,0xbb,0x01, 0x02, 5, 1,2,3,4,5];

    let apdu = Apdu::new_fixed(&mut apdu_bin).ok().unwrap();

    assert!(apdu.case == 3);
    assert!(apdu.cla == 0xaa);
    assert!(apdu.ins == 0xbb);
    assert!(apdu.p1 == 0x01);
    assert!(apdu.p2 == 0x02);

    assert!(apdu.lc == 5);
    assert!(apdu.le == 0);

    let mut sum = 0;
    for i in 0 .. apdu.lc {  sum += apdu.buffer[ apdu.offset + i as usize];  }
    assert!(sum == (5+4+3+2+1));
}

#[test]
fn test_apdu_case_4() {
    let mut apdu_bin: [u8; 5 + 5 + 1] = [0xaa,0xbb,0x01, 0x02, 5, 1,2,3,4,5, 100];

    let apdu = Apdu::new_fixed(&mut apdu_bin).ok().unwrap();

    assert!(apdu.case == 4);
    assert!(apdu.cla == 0xaa);
    assert!(apdu.ins == 0xbb);
    assert!(apdu.p1 == 0x01);
    assert!(apdu.p2 == 0x02);

    assert!(apdu.lc == 5);
    assert!(apdu.le == 100);
}

#[test]
fn test_apdu_case_2e() {
    let mut apdu_bin: [u8; 7] = [0xaa,0xbb,0x01, 0x02, 0x00, 0x12, 0x34];

    let apdu = Apdu::new_fixed(&mut apdu_bin).ok().unwrap();

    assert!(apdu.case == 0x12);
    assert!(apdu.cla == 0xaa);
    assert!(apdu.ins == 0xbb);
    assert!(apdu.p1 == 0x01);
    assert!(apdu.p2 == 0x02);

    assert!(apdu.lc == 0);
    assert!(apdu.le == 0x1234);
}

#[test]
fn test_apdu_case_3e() {
    let mut apdu_bin: [u8; 0x123 + 7] = [0xaa,0xbb,0x01, 0x02, 0x00, 0x01, 0x23,
        1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        3,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        5,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        6,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        7,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        9,0,0,0,0,0,0,0,0,0,0,
    ];

    let apdu = Apdu::new_fixed(&mut apdu_bin).ok().unwrap();

    assert!(apdu.case == 0x13);
    assert!(apdu.cla == 0xaa);
    assert!(apdu.ins == 0xbb);
    assert!(apdu.p1 == 0x01);
    assert!(apdu.p2 == 0x02);

    assert!(apdu.lc == 0x123);
    assert!(apdu.le == 0x00);

    let mut sum = 0;
    for i in 0 .. apdu.lc {  sum += apdu.buffer[ apdu.offset + i as usize];  }
    assert!(sum == (9+8+7+6+5+4+3+2+1));
    
}


#[test]
fn test_apdu_case_4e() {
    let mut apdu_bin: [u8; 0x123 + 7 + 2] = [0xaa,0xbb,0x01, 0x02, 0x00, 0x01, 0x23,
        1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        3,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        5,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        6,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        7,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        9,0,0,0,0,0,0,0,0,0,0,0x45, 0x67
    ];

    let apdu = Apdu::new_fixed(&mut apdu_bin).ok().unwrap();

    assert!(apdu.case == 0x14);
    assert!(apdu.cla == 0xaa);
    assert!(apdu.ins == 0xbb);
    assert!(apdu.p1 == 0x01);
    assert!(apdu.p2 == 0x02);

    assert!(apdu.lc == 0x123);
    assert!(apdu.le == 0x4567);

    let mut sum = 0;
    for i in 0 .. apdu.lc {  sum += apdu.buffer[ apdu.offset + i as usize];  }
    assert!(sum == (9+8+7+6+5+4+3+2+1));
    
}

#[test]
fn test_apdu_case_4e_extra_byte() {
    let mut apdu_bin: [u8; 0x123 + 7 + 3] = [0xaa,0xbb,0x01, 0x02, 0x00, 0x01, 0x23,
        1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        3,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        5,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        6,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        7,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        9,0,0,0,0,0,0,0,0,0,0, 0x00, 0x45, 0x67
    ];

    let apdu = Apdu::new_fixed(&mut apdu_bin).ok().unwrap();

    assert!(apdu.case == 0x14);
    assert!(apdu.cla == 0xaa);
    assert!(apdu.ins == 0xbb);
    assert!(apdu.p1 == 0x01);
    assert!(apdu.p2 == 0x02);

    assert!(apdu.lc == 0x123);
    assert!(apdu.le == 0x4567);

    let mut sum = 0;
    for i in 0 .. apdu.lc {  sum += apdu.buffer[ apdu.offset + i as usize];  }
    assert!(sum == (9+8+7+6+5+4+3+2+1));
    
}


#[test]
fn test_apdu_case_3e_missing_bytes() {
    let mut apdu_bin: [u8; 0x123 + 7 -1] = [0xaa,0xbb,0x01, 0x02, 0x00, 0x01, 0x23,
        1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        3,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        5,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        6,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        7,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        9,0,0,0,0,0,0,0,0,0,
    ];

    assert!( Apdu::new_fixed(&mut apdu_bin).err().unwrap() == Error::SwWrongLength);

    let mut apdu_bin: [u8; 7 +1] = [0xaa,0xbb,0x01, 0x02, 0x00, 0x01, 0x23, 0x00
    ];

    assert!( Apdu::new_fixed(&mut apdu_bin).err().unwrap() == Error::SwWrongLength);
}

#[test]
fn test_apdu_case_3e_too_many_bytes() {
    let mut apdu_bin: [u8; 0x123 + 7 -1 + 35] = [0xaa,0xbb,0x01, 0x02, 0x00, 0x01, 0x23,
        1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        3,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        5,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        6,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        7,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        9,0,0,0,0,0,0,0,0,0,
    ];

    assert!( Apdu::new_fixed(&mut apdu_bin).err().unwrap() == Error::SwWrongLength);

}

#[test]
fn test_apdu_bad_case() {
    let mut apdu_bin: [u8; 0x03] = [0xaa,0xbb,0x01];

    assert!( Apdu::new_fixed(&mut apdu_bin).err().unwrap() == Error::SwWrongLength);
}


#[test]
fn test_apdu_case_3_missing_bytes() {
    let mut apdu_bin: [u8; 9 - 1] = [0xaa,0xbb,0x01,0x02,0x05, 1, 2, 3];

    assert!( Apdu::new_fixed(&mut apdu_bin).err().unwrap() == Error::SwWrongLength);
}

#[test]
fn test_apdu_case_3_too_many_bytes() {
    let mut apdu_bin: [u8; 9 + 9] = [0xaa,0xbb,0x01,0x02,0x05, 1, 2, 3, 4,
        1,2,3,4,5,6,7,8,9,
    ];

    assert!( Apdu::new_fixed(&mut apdu_bin).err().unwrap() == Error::SwWrongLength);
}


