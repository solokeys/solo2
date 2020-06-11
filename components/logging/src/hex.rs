
pub trait HexRepresentation2 {
    fn hex(self) -> [u8; 2];
}
pub trait HexRepresentation4 {
    fn hex(self) -> [u8; 4];
}
pub trait HexRepresentation8 {
    fn hex(self) -> [u8; 4];
}

impl HexRepresentation2 for u8 {
    fn hex(self) -> [u8; 2] {
        let mut hex = [0x30, 0x30];

        for i in 0 .. 2 {
            let nibble = (self >> (i * 4)) & 0xf;
            hex[1-i] = if  nibble < 0x0a {
                nibble + 0x30
            }
            else
            {
                nibble + 0x41 - 0x0A
            }
        }
        hex
    }
}

impl HexRepresentation4 for u16{
    fn hex(self) -> [u8; 4] {
        let mut hex = [0x30, 0x30, 0x30, 0x30];

        let bottom = ((self & 0xff) as u8).hex();
        let top = (((self & 0xff00)>>8) as u8).hex();

        hex[0] = top[0];
        hex[1] = top[1];
        hex[2] = bottom[0];
        hex[3] = bottom[1];

        hex
    }
}

#[macro_export]
macro_rules! hex {
    ($byte:expr) => {
        unsafe{ core::str::from_utf8_unchecked(&($byte).hex()) }
    }
}

