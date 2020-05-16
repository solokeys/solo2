pub use untrusted::{Input, Reader};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    HighTagNumberForm,
    LongLengthNotSupported,
    NonCanonical,
    Read,
    UnexpectedEnd,
    WrongTag,
    WrongValue,
}

pub type Result<T> = core::result::Result<T, Error>;

impl From<untrusted::EndOfInput> for Error {
    fn from(_: untrusted::EndOfInput) -> Error {
        Error::UnexpectedEnd
    }
}

/// Return the value of the given tag and apply a decoding function to it.
pub fn nested<'a, F, R>(input: &mut Reader<'a>, tag: u8, decoder: F) -> Result<R>
where
    F: FnOnce(&mut untrusted::Reader<'a>) -> Result<R>,
{
    let inner = expect_tag_and_get_value(input, tag)?;
    inner.read_all(Error::Read, decoder)
}

/// Read a tag and return it's value. Errors when the expect and actual tag do not match.
pub fn expect_tag_and_get_value<'a>(input: &mut Reader<'a>, tag: u8) -> Result<Input<'a>> {
    let (actual_tag, inner) = read_tag_and_get_value(input)?;
    if usize::from(tag) != usize::from(actual_tag) {
        return Err(Error::WrongTag);
    }
    Ok(inner)
}

/// Read a tag and its value. Errors when the expected and actual tag and values do not match.
pub fn expect_tag_and_value<'a>(input: &mut Reader<'a>, tag: u8, value: &[u8]) -> Result<()> {
    let (actual_tag, inner) = read_tag_and_get_value(input)?;
    if usize::from(tag) != usize::from(actual_tag) {
        return Err(Error::WrongTag);
    }
    if value != inner.as_slice_less_safe() {
        return Err(Error::WrongValue);
    }
    Ok(())
}

/// Read the next tag, and return it and its value.
pub fn read_tag_and_get_value<'a>(input: &mut Reader<'a>) -> Result<(u8, Input<'a>)> {
    let tag = input.read_byte()?;
    if (tag & 0x1F) == 0x1F {
        return Err(Error::HighTagNumberForm);
    }

    // If the high order bit of the first byte is set to zero then the length
    // is encoded in the seven remaining bits of that byte. Otherwise, those
    // seven bits represent the number of bytes used to encode the length.
    let length = match input.read_byte()? {
        n if (n & 0x80) == 0 => usize::from(n),
        0x81 => {
            let second_byte = input.read_byte()?;
            if second_byte < 128 {
                return Err(Error::NonCanonical);
            }
            usize::from(second_byte)
        }
        0x82 => {
            let second_byte = usize::from(input.read_byte()?);
            let third_byte = usize::from(input.read_byte()?);
            let combined = (second_byte << 8) | third_byte;
            if combined < 256 {
                return Err(Error::NonCanonical);
            }
            combined
        }
        _ => return Err(Error::LongLengthNotSupported),
    };

    let inner = input.read_bytes(length)?;
    Ok((tag, inner))
}
