//! Convenient `Display` and other traits for binary data.
//!
//! Standard Rust uses the `fmt::UpperHex` and `LowerHex` traits to implement hexadecimal
//! representations for use in format strings. For instance,
//! `format_args!("{:02X?}", &[7, 0xA1, 0xFF])` produces the string `"[07, A1, FF]"`.
//!
//! ```
//! assert_eq!(format!("{:02X?}", [7u8, 0xA1, 0xFF].as_ref()), "[07, A1, FF]");
//! ```
//!
//! However, this only works for the `Debug` trait, not `Display`; needs extra dancing
//! to pad with leading zeros, and is not very compact when debugging binary data formats.
//!
//! The idea of this module (and the `hex_fmt` crate, of which `HexFmt` and `HexList` are
//! re-exported), is to generate newtypes around byte arrays/slices with implementations of
//! the `fmt` traits. In release mode, this is all compiled out and translates to direct
//! instructions for the formatting machinery.
//!
//! Three examples (the parameter `2` to `hex_str` in the fourth example denotes "blocks of 2 bytes"):
//!
//! ```
//! use delog::{hex_str, hexstr};
//!
//! // Use the following at your crate's root instead to "pierce" namespaces
//! // #[macro_use]
//! // extern crate delog;
//!
//! let four_bytes = &[7u8, 0xA1, 255, 0xC7];
//!
//! assert_eq!(format!("{}", hexstr!(four_bytes)), "07A1FFC7");
//! assert_eq!(format!("{:x}", hexstr!(four_bytes)), "07a1ffc7");
//! assert_eq!(format!("{:x}", hex_str!(four_bytes)), "07 a1 ff c7");
//! assert_eq!(format!("{}", hex_str!(four_bytes, 2)), "07A1 FFC7");
//! ```

use core::marker::PhantomData;
use core::fmt;

use hex_fmt;

/// re-export from `hex_fmt`
///
pub use hex_fmt::HexFmt;
/// re-export from `hex_fmt`
///
pub use hex_fmt::HexList;

pub use typenum::{consts, Unsigned};

/// A type that specifies a separator str.
pub trait Separator {
    const SEPARATOR: &'static str;
}

/// Parameter to `HexStr` to suppress separators between hexadecimal blocks.
pub struct NullSeparator {}
impl Separator for NullSeparator {
    const SEPARATOR: &'static str = "";
}
/// Parameter to `HexStr` to separate hexadecimal blocks with spaces.
pub struct SpaceSeparator {}
impl Separator for SpaceSeparator {
    const SEPARATOR: &'static str = " ";
}

/// Zero-sized wrapper newtype, allowing grouping bytes in blocks of N hexadecimals
/// during formatting.
///
/// Use the method with the same name to construct this from your byte array or slice,
/// or preferrably the `hex_str!` or `hexstr!` macro.
pub struct HexStr<'a, T: ?Sized, S=SpaceSeparator, BytesPerBlock=consts::U1>
where
    S: Separator,
    BytesPerBlock: Unsigned,
{
    pub value: &'a T,
    _separator: PhantomData<S>,
    _block_size: PhantomData<BytesPerBlock>,
}

#[macro_export]
/// Compactly format byte arrays and slices as hexadecimals.
///
/// Exposes the `hex_str` function as macro, for ease of practical use via the
/// latter's "namespace-piercing" capabilities.
///
/// The second parameter refers to the number of bytes in a block (separated by spaces).
///
/// ```
/// use delog::hex_str;
/// let four_bytes = &[7u8, 0xA1, 255, 0xC7];
/// assert_eq!(format!("{:x}", hex_str!(four_bytes)), "07 a1 ff c7");
/// assert_eq!(format!("{}", hex_str!(four_bytes, 2)), "07A1 FFC7");
/// assert_eq!(format!("{}", hex_str!(four_bytes, 3)), "07A1FF C7");
/// ```
macro_rules! hex_str {
    ($array:expr) => { $crate::hex::hex_str($array) };
    ($array:expr, 1) => { $crate::hex::hex_str_1($array) };
    ($array:expr, 2) => { $crate::hex::hex_str_2($array) };
    ($array:expr, 3) => { $crate::hex::hex_str_3($array) };
    ($array:expr, 4) => { $crate::hex::hex_str_4($array) };
    ($array:expr, 5) => { $crate::hex::hex_str_5($array) };
    ($array:expr, 8) => { $crate::hex::hex_str_8($array) };
    ($array:expr, 16) => { $crate::hex::hex_str_16($array) };
    ($array:expr, 20) => { $crate::hex::hex_str_20($array) };
    ($array:expr, 32) => { $crate::hex::hex_str_32($array) };
    ($array:expr, 64) => { $crate::hex::hex_str_64($array) };
}

#[macro_export]
/// More compactly format byte arrays and slices as hexadecimals.
///
/// Exposes the `hexstr` function (no spaces) as macro, for ease of practical use via the
/// latter's "namespace-piercing" capabilities.
///
/// ```
/// use delog::hexstr;
/// let four_bytes = &[7u8, 0xA1, 255, 0xC7];
/// assert_eq!(format!("{}", hexstr!(four_bytes)), "07A1FFC7");
/// assert_eq!(format!("{:x}", hexstr!(four_bytes)), "07a1ffc7");
/// ```
macro_rules! hexstr {
    ($array:expr) => { $crate::hex::hexstr($array) };
}

#[allow(non_snake_case)]
/// dive into `typenum` and discover your inner traitist
///
/// The first parameter denotes the separator, the second `typenum` parameter
/// denotes the block size in bytes, e.g. `consts::U7` means blocks of 7 bytes (or 56 bits).
///
/// In most cases, using one of the macros will suffice and is preferrable.
///
/// ```
/// use delog::hex::{HexStr, Separator, consts};
/// struct Pipe {}
/// impl Separator for Pipe {
///     const SEPARATOR: &'static str  = "|";
/// }
/// let four_bytes = &[7u8, 0xA1, 255, 0xC7];
/// let hex_str = HexStr::<_, Pipe, consts::U3>(four_bytes);
/// assert_eq!(format!("{}", hex_str), "07A1FF|C7");
/// ```
pub fn HexStr<'a, T: ?Sized, S: Separator, B: Unsigned>(value: &'a T) -> HexStr<'a, T, S, B> {
    HexStr { value, _separator: PhantomData, _block_size: PhantomData }
}

/// blocks of 1 byte / 8 bits in hex, no space in between (e.g., `4A121387`)
///
/// For ease of use, prefer the `hexstr!(value)` macro.
pub fn hexstr<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, NullSeparator, consts::U1> {
    HexStr(value)
}

/// synonym for `hex_str_1`, like ISO 7816 if enclosed in single quotes (`'8A 4F 12 AA'`).
///
/// For ease of use, prefer the `hex_str!(value)` macro.
pub fn hex_str<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T> {
    HexStr(value)
}

/// blocks of 1 byte / 8 bits in hex, space in between (e.g., `4A 12 13 87`)
///
/// For ease of use, prefer the `hex_str!(value)` macro.
pub fn hex_str_1<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U1> {
    HexStr(value)
}

/// blocks of 2 bytes / 16 bits in hex, space in between (e.g., `4A12 1387`)
///
/// For ease of use, prefer the `hex_str!(value, 2)` macro.
pub fn hex_str_2<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U2> {
    HexStr(value)
}

/// blocks of 3 bytes / 24 bits in hex, space in between (e.g., `4A1213 871234 ABCD`)
///
/// For ease of use, prefer the `hex_str!(value, 3)` macro.
pub fn hex_str_3<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U3> {
    HexStr(value)
}

/// blocks of 4 bytes / 32 bits in hex, space in between (e.g., `4A121387 1234ABCD`)
///
/// For ease of use, prefer the `hex_str!(value, 4)` macro.
pub fn hex_str_4<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U4> {
    HexStr(value)
}

/// blocks of 5 bytes / 40 bits in hex, space in between (e.g., `4A12138712 34ABCD`)
///
/// For ease of use, prefer the `hex_str!(value, 5)` macro.
pub fn hex_str_5<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U5> {
    HexStr(value)
}

/// blocks of 8 bytes / 64 bits in hex, space in between
///
/// For ease of use, prefer the `hex_str!(value, 8)` macro.
pub fn hex_str_8<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U8> {
    HexStr(value)
}

/// blocks of 16 bytes / 128 bits in hex, space in between
///
/// For ease of use, prefer the `hex_str!(value, 16)` macro.
pub fn hex_str_16<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U16> {
    HexStr(value)
}

/// blocks of 20 bytes / 160 bits in hex, space in between
///
/// For ease of use, prefer the `hex_str!(value, 20)` macro.
pub fn hex_str_20<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U20> {
    HexStr(value)
}

/// blocks of 32 bytes / 256 bits in hex, space in between
///
/// For ease of use, prefer the `hex_str!(value, 32)` macro.
pub fn hex_str_32<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U32> {
    HexStr(value)
}

/// blocks of 64 bytes / 512 bits in hex, space in between
///
/// For ease of use, prefer the `hex_str!(value, 64)` macro.
pub fn hex_str_64<'a, T: ?Sized>(value: &'a T) -> HexStr<'a, T, SpaceSeparator, consts::U64> {
    HexStr(value)
}

impl<T, S, U> fmt::Debug for HexStr<'_, T, S, U>
where
    T: AsRef<[u8]>,
    S: Separator,
    U: Unsigned,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt::Display::fmt(self, f)
    }
}

impl<T, S, U> fmt::Display for HexStr<'_, T, S, U>
where
    T: AsRef<[u8]>,
    S: Separator,
    U: Unsigned,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt::UpperHex::fmt(self, f)
    }
}

macro_rules! implement {
    ($Trait:ident, $padded_formatter:expr) => {
        impl<'a, T: ?Sized, S, U> fmt::$Trait for HexStr<'a, T, S, U>
        where
            T: AsRef<[u8]>,
            S: Separator,
            U: Unsigned,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                let mut first = true;
                for entry in self.value.as_ref().chunks(U::USIZE) {
                    if !first {
                        f.write_str(S::SEPARATOR)?;
                    } else {
                        first = false;
                    }
                    for byte in entry.iter() {
                        write!(f, $padded_formatter, byte)?;
                        // fmt::$Trait::fmt(byte, f)?;
                    }
                }
                Ok(())
            }
        }
    }
}

implement!(LowerHex, "{:02x}");
implement!(UpperHex, "{:02X}");

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_hex_str() {
        let buf = [1u8, 2, 3, 0xA1, 0xB7, 0xFF, 0x3];
        insta::assert_debug_snapshot!(format_args!("'{:02X}'", hex_str_1(&buf)));
        insta::assert_debug_snapshot!(format_args!("'{:02X}'", hex_str_2(&buf)));
        insta::assert_debug_snapshot!(format_args!("'{:02x}'", hex_str_2(&buf)));
        insta::assert_debug_snapshot!(format_args!("'{:02X}'", hex_str_4(&buf)));
        insta::assert_debug_snapshot!(format_args!("'{:02X}'", hex_str_4(&buf[..])));
        insta::assert_debug_snapshot!(format_args!("'{:02X}'", hex_str_4(&buf)));
        insta::assert_debug_snapshot!(format_args!("'{:X}'", hex_str_4(&buf)));
    }

    #[test]
    fn test_custom_hex_str() {
        let buf = [1u8, 2, 3, 0xA1, 0xB7, 0xFF, 0x3];
        insta::assert_debug_snapshot!(format_args!(
            "'{:02X}'",
            HexStr::<_, SpaceSeparator, consts::U3>(&buf),
        ));
    }

}
