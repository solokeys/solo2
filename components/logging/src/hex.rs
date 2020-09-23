
pub trait HexRepresentation2 {
    fn as_bytes(self) -> [u8; 2];
    // fn hex_string(self) -> &'static str{
        // unsafe{ core::str::from_utf8_unchecked(&(self).hex()) }
    // }
    fn hex(self) -> &'static str;

}
pub trait HexRepresentation4 {
    fn as_bytes(self) -> [u8; 4];
    // fn hex_string(self) -> &'static str{
        // unsafe{ core::str::from_utf8_unchecked(&(self).hex()) }
    // }
    fn hex(self) -> &'static str;
}
pub trait HexRepresentation8 {
    fn as_bytes(self) -> [u8; 4];
    fn hex(self) -> &'static str;
}

impl HexRepresentation2 for u8 {
    fn as_bytes(self) -> [u8; 2] {
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

    fn hex(self) -> &'static str{
        static mut MEM: [u8; 2] = [0,0];
        unsafe{ MEM = self.as_bytes() };
        unsafe{ core::str::from_utf8_unchecked(&MEM) }
    }

}

impl HexRepresentation4 for u16{
    fn as_bytes(self) -> [u8; 4] {
        let mut hex = [0x30, 0x30, 0x30, 0x30];

        let bottom = ((self & 0xff) as u8).as_bytes();
        let top = (((self & 0xff00)>>8) as u8).as_bytes();

        hex[0] = top[0];
        hex[1] = top[1];
        hex[2] = bottom[0];
        hex[3] = bottom[1];

        hex
    }
    fn hex(self) -> &'static str{
        static mut MEM: [u8; 4] = [0,0,0,0];
        unsafe{ MEM = self.as_bytes() };
        unsafe{ core::str::from_utf8_unchecked(&MEM) }
    }
}

#[macro_export]
macro_rules! hex {
    ($byte:expr) => {
        unsafe{ core::str::from_utf8_unchecked(&($byte).as_bytes()) }
    }
}


