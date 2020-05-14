pub mod class;
pub mod instruction;

pub type Data = heapless_bytes::Bytes<crate::MAX_COMMAND_DATA>;

#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    class: class::Class,
    instruction: instruction::Instruction,

    pub p1: u8,
    pub p2: u8,

    /// The main reason this is modeled as ByteBuf and not
    /// a fixed array is for serde purposes.
    data: Data,

    le: usize,
    pub extended: bool,
}

impl Command {
    pub fn try_from(apdu: &[u8]) -> Result<Self, FromSliceError> {
        use core::convert::TryInto;
        apdu.try_into()
    }

    pub fn class(&self) -> class::Class {
        self.class
    }

    pub fn instruction(&self) -> instruction::Instruction {
        self.instruction
    }

    pub fn data(&self) -> &Data {
        &self.data
    }

    pub fn expected(&self) -> usize {
        self.le
    }

    // pub fn instruction(&self) -> class::Class {
    // }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FromSliceError {
    TooShort,
    InvalidClass,
    InvalidFirstBodyByteForExtended,
    CanThisReallyOccur,
}

impl From<class::InvalidClass> for FromSliceError {
    fn from(_: class::InvalidClass) -> Self {
        Self::InvalidClass
    }
}

impl core::convert::TryFrom<&[u8]> for Command {
    type Error = FromSliceError;
    fn try_from(apdu: &[u8]) -> core::result::Result<Self, Self::Error> {
        if apdu.len() < 4 {
            return Err(FromSliceError::TooShort);
        }
        let (header, body) = apdu.split_at(4);
        let class = class::Class::try_from(header[0])?;
        let instruction = instruction::Instruction::from(header[1]);
        let parsed = parse_lengths(body)?;
        let data_slice = &body[parsed.offset..][..parsed.lc];

        Ok(Self {
            class,
            instruction,
            p1: header[2],
            p2: header[3],
            le: parsed.le,
            data: Data::try_from_slice(data_slice).unwrap(),
            extended: parsed.extended,
        })
    }
}

// cf. ISO 7816-3, 12.1.3: Decoding conventions for command APDUs
// freely available version:
// http://www.ttfn.net/techno/smartcards/iso7816_4.html#table5

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct ParsedLengths {
    lc: usize,
    le: usize,
    offset: usize,
    extended: bool,
}

#[inline(always)]
fn replace_zero(value: usize, replacement: usize) -> usize {
    if value == 0 {
        replacement
    } else {
        value
    }
}
#[inline]
fn parse_lengths(body: &[u8]) -> Result<ParsedLengths, FromSliceError> {

    // Encoding rules:
    // - Lc or Le = 0 => leave out
    // - short + extended length fields shall not be combined
    // - for extended, if Lc > 0, then Le has no leading zero byte

    let l = body.len();

    let mut parsed: ParsedLengths = Default::default();

    // Case 1
    if l == 0 {
        return Ok(parsed);
    }

    // the reference starts indexing at 1
    let b1 = body[0] as usize;

    // Case 2S
    if l == 1 {
        parsed.lc = 0;
        parsed.le = replace_zero(b1, 256);
        return Ok(parsed)
    }

    // Case 3S
    if l == 1 + b1 && b1 != 0 {
        // B1 encodes Lc valued from 1 to 255
        parsed.lc = b1;
        parsed.le = 0;
        parsed.offset = 1;
        return Ok(parsed);
    }

    // Case 4S
    if l == 2 + b1 && b1 != 0 {
        // B1 encodes Lc valued from 1 to 255
        // Bl encodes Le from 1 to 256
        parsed.lc = b1;
        parsed.le = replace_zero(body[l - 1] as usize + 1, 256);
        parsed.offset = 1;
        return Ok(parsed);
    }

    parsed.extended = true;

    // only extended cases left now
    if b1 != 0 {
        return Err(FromSliceError::InvalidFirstBodyByteForExtended);
    };

    // Case 2E (no data)
    if l == 3 && b1 == 0 {
        parsed.lc = 0;
        parsed.le = replace_zero(
            u16::from_be_bytes([body[1], body[2]]) as usize,
            65_536);
        return Ok(parsed);
    }

    parsed.lc = u16::from_be_bytes([body[1], body[2]]) as usize;

    // Case 3E
    if l == 3 + parsed.lc {
        parsed.le = 0;
        parsed.offset = 3;
        return Ok(parsed);
    }

    // Case 4E
    if l == 5 + parsed.lc {
        parsed.le =  replace_zero(
            u16::from_be_bytes([body[l - 2], body[l - 1]]) as usize,
            65_536);
        parsed.offset = 3;
        return Ok(parsed);
    }

    Err(FromSliceError::CanThisReallyOccur)
}
