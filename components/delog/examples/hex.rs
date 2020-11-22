use delog::hex::*;

// these examples are also `insta` tests,
// see <src/snapshots> for expected outputs
fn main() {
    let buf = [1u8, 2, 3, 0xA1, 0xB7, 0xFF, 0x3];
    println!("'{:02X}'", hex_str_1(&buf));
    println!("'{:02X}'", hex_str_2(&buf));
    println!("'{:02x}'", delog::hex_str!(&buf, 2));
    println!("'{:02X}'", hex_str_4(&buf));
    println!("'{:02X}'", hex_str_4(&buf[..]));
    println!("'{:X}'", hex_str_4(&buf));
}
