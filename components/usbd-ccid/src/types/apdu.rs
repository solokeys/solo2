use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub cla: u8,
    pub ins: u8,
    pub p1: u8,
    pub p2: u8,
    pub lc: usize,
    pub le: usize,
    pub data: MessageBuffer,
}

impl core::convert::TryFrom<&MessageBuffer> for Command {
    type Error = ();
    fn try_from(message: &MessageBuffer) -> core::result::Result<Self, Self::Error> {
        let apdu = Apdu::try_from(message.as_ref())?;
        Ok(Self {
            cla: apdu.cla(),
            ins: apdu.ins(),
            p1: apdu.p1(),
            p2: apdu.p2(),
            lc: apdu.lc(),
            le: apdu.le(),
            data: MessageBuffer::try_from_slice(apdu.data()).unwrap(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Response {
    pub sw1: u8,
    pub sw2: u8,
    pub data: MessageBuffer,
}

impl Response {
    pub fn into_message(self) -> MessageBuffer {
        let mut message = MessageBuffer::new();
        message.extend_from_slice(&self.data).unwrap();
        message.push(self.sw1).unwrap();
        message.push(self.sw2).unwrap();
        message
    }
}

interchange::interchange! {
    ApduInterchange: (Command, Response)
}

pub struct Apdu<'a> {
    lc: usize,
    le: usize,
    offset: usize,
    apdu: &'a [u8]
}

impl<'a> core::ops::Deref for Apdu<'a> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.apdu
    }
}

impl<'a> core::convert::TryFrom<&'a [u8]> for Apdu<'a> {
    type Error = ();
    fn try_from(apdu: &'a [u8]) -> core::result::Result<Self, Self::Error> {
        let (lc, le, offset) = calculate_lengths(apdu)?;
        Ok(Self { lc, le, offset, apdu })
    }
}

// There are four ranges:
// First Interindustry   0b000x_xxxx
// Further Interindustry 0b01xx_xxxx
// Reserved              0b001x_xxxx
// Proprietary           0b1xxx_xxxx
//
// For the interindustry ranges, class contains:
// - chaining (continues/last)
// - secure messaging indication (none, two standard, proprietary)
// - logical channel number
pub struct Class {
    cla: u8
}

impl core::ops::Deref for Class {
    type Target = u8;
    fn deref(&self) -> &Self::Target {
        &self.cla
    }
}

pub enum Range {
    FirstInterindustry,
    FurtherInterindustry,
    Reserved,
    Proprietary,
    Invalid,
}

pub enum SecureMessaging {
    None = 0,
    Proprietary = 1,
    Standard = 2,
    Authenticated = 3,
    Unknown,
}

pub enum Chain {
    LastOrOnly,
    NotTheLast,
    Unknown,
}

impl Class {
    #[inline]
    pub fn last(&self) -> bool {
        self.cla & (1 << 4) == 0
    }

    #[inline]
    pub fn chain(&self) -> Chain {
        match self.range() {
            Range::FirstInterindustry | Range::FurtherInterindustry=> {
                if self.cla & (1 << 4) != 0 {
                    Chain::NotTheLast
                } else {
                    Chain::LastOrOnly
                }
            }
            _ => Chain::Unknown,
        }
    }

    #[inline]
    pub fn channel(&self) -> Option<u8> {
        Some(match self.range() {
            Range::FirstInterindustry => {
                self.cla & 0b11
            }
            Range::FurtherInterindustry => {
                4 + self.cla & 0b111
            }
            _ => return None
        })
    }

    #[inline]
    pub fn range(&self) -> Range {
        if self.cla == 0xff {
            return Range::Invalid;
        }
        match self.cla >> 5 {
            0b000 => Range::FirstInterindustry,
            0b010 | 0b011 => Range::FurtherInterindustry,
            0b001 => Range::Reserved,
            _ => Range::Proprietary,
        }
    }

    #[inline]
    pub fn secure_messaging(&self) -> SecureMessaging {
        match self.range() {
            Range::FirstInterindustry => {
                match (self.cla >> 2) & 0b11 {
                    0b00 => SecureMessaging::None,
                    0b01 => SecureMessaging::Proprietary,
                    0b10 => SecureMessaging::Standard,
                    0b11 => SecureMessaging::Authenticated,
                    _ => unreachable!(),
                }
            }
            Range::FurtherInterindustry => {
                match (self.cla >> 5)  != 0 {
                    true => SecureMessaging::Standard,
                    false => SecureMessaging::None,
                }
            }
            _ => SecureMessaging::Unknown,
        }
    }
}

impl Apdu<'_> {
    #[inline]
    /// The "class" byte of the APDU
    pub fn cla(&self) -> u8 {
        *&self[0]

    }

    #[inline]
    /// The "instruction" byte of the APDU
    pub fn ins(&self) -> u8 {
        *&self[1]
    }

    #[inline]
    /// The first "parameter" byte of the APDU
    pub fn p1(&self) -> u8 {
        *&self[2]
    }

    #[inline]
    /// The second "parameter" byte of the APDU
    pub fn p2(&self) -> u8 {
        *&self[3]
    }

    #[inline]
    /// The length of the APDU's command data bytes
    pub fn lc(&self) -> usize {
        self.lc
    }

    #[inline]
    /// The maximum expected length of the response
    pub fn le(&self) -> usize {
        self.le
    }

    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.apdu[4 + self.offset..][..self.lc]
    }

}

impl core::fmt::Debug for Apdu<'_> {

    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {

        let mut debug_struct = f.debug_struct("Apdu");

        let mut debug_struct = debug_struct
            .field("CLA", &format_args!("0x{:x}", &self.cla()))
            .field("INS", &format_args!("0x{:x}", &self.ins()))
            .field("P1", &format_args!("0x{:x}", &self.p1()))
            .field("P2", &format_args!("0x{:x}", &self.p2()))
            .field("Lc", &self.lc())
            .field("Le", &self.le())
        ;

        if self.lc() > 0 {
            let l = core::cmp::min(self.lc(), 8);
            debug_struct = if l > 8 {
                debug_struct
                    .field("data[..8]", &(&self.data()[..8]))
            } else {
                debug_struct
                    .field("data", &self.data())
            }
        };

        debug_struct.finish()
    }
}

// cf. ISO 7816-3, 12.1.3: Decoding conventions for command APDUs
// freely available version:
// http://www.ttfn.net/techno/smartcards/iso7816_4.html#table5

#[inline]
fn calculate_lengths(apdu: &[u8]) -> Result<(usize, usize, usize), ()> {

    // Encoding rules:
    // - Lc or Le = 0 => leave out
    // - short + extended length fields shall not be combined
    // - for extended, if Lc > 0, then Le has no leading zero byte
    hprintln!("parsing {:?}", apdu).ok();
    // b = body
    let b = &apdu[4..];
    let l = b.len();
    let mut le: usize;
    let mut lc: usize;

    let mut offset: usize = 0;

    // Case 1
    if l == 0{
        lc = 0;
        le = 0;
        return Ok((lc, le, offset));
    }

    // the reference use indexing-from-1
    let b1 = b[0] as usize;

    // Case 2S
    if l == 1 {
        lc = 0;
        le = if b1 == 0 {
            256
        } else {
            b1 as _
        };
        return Ok((lc, le, offset));
    }

    // Case 3S
    if l == 1 + b1 && b1 != 0 {
        // B1 encodes Lc valued from 1 to 255
        lc = b1;
        le = 0;
        return Ok((lc, le, 1));
    }

    // Case 4S
    if l == 2 + b1 && b1 != 0 {
        // B1 encodes Lc valued from 1 to 255
        // Bl encodes Le from 1 to 256
        lc = b1;
        le = if b[l - 1] == 0 {
            256
        } else {
            b[l - 1] as usize + 1
        };
        return Ok((lc, le, 1));
    }

    // only extended cases left now
    if b1 != 0 {
        return Err(())
    };

    // Case 2E (no data)
    if l == 3 && b1 == 0 {
        lc = 0;
        if b[1] == 0 && b[2] == 0 {
            le = 65_536;
        } else {
            le = u16::from_be_bytes([b[1], b[2]]) as usize;
        }
        return Ok((lc, le, 0));
    }

    lc = u16::from_be_bytes([b[1], b[2]]) as usize;

    // Case 3E
    if l == 3 + lc {
        le = 0;
        return Ok((lc, le, 3));
    }

    // Case 4E
    if l == 5 + lc {
        let pre_le = u16::from_be_bytes([b[l - 2], b[l - 1]]) as usize;
        if pre_le == 0 {
            le = 65_6536;
        } else {
            le = pre_le;
        }
        return Ok((lc, le, 3));
    }

    Err(())
}


// // pub trait Apdu: core::ops::Deref<Target = RawPacket> {
// pub trait Apdu<'a>: core::ops::Deref<Target = &'a [u8]> {
// }
