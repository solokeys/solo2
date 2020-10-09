pub use heapless::{consts, ArrayLength, ByteBuf};

const CONSTRUCTED: u8 = 1 << 5;
// const CONTEXT_SPECIFIC: u8 = 2 << 6;

/// ASN.1 Tags
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Tag {
    // Eoc = 0x00,
    // Boolean = 0x01,
    Integer = 0x02,
    // BitString = 0x03,
    // OctetString = 0x04,
    // Null = 0x05,
    // Oid = 0x06,
    Sequence = CONSTRUCTED | 0x10,
    // UtcTime = 0x17,
    // GeneralizedTime = 0x18,
    // ContextSpecificConstructed0 = CONTEXT_SPECIFIC | CONSTRUCTED | 0,
    // ContextSpecificConstructed1 = CONTEXT_SPECIFIC | CONSTRUCTED | 1,
    // ContextSpecificConstructed2 = CONTEXT_SPECIFIC | CONSTRUCTED | 2,
    // ContextSpecificConstructed3 = CONTEXT_SPECIFIC | CONSTRUCTED | 3,
}

// impl From<Tag> for usize {
//     fn from(tag: Tag) -> Self {
//         tag as Self
//     }
// }

// impl From<Tag> for u8 {
//     fn from(tag: Tag) -> Self {
//         tag as Self
//     }
// }

// the only error is buffer overflow
type Result = core::result::Result<(), ()>;

/// DER writer
#[derive(Debug)]
pub struct Der<N>(ByteBuf<N>)
where
    N: ArrayLength<u8>;

impl<N: ArrayLength<u8>> Default for Der<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N: ArrayLength<u8>> core::ops::Deref for Der<N> {
    type Target = ByteBuf<N>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<N: ArrayLength<u8>> core::ops::DerefMut for Der<N> {
    fn deref_mut(&mut self) -> &mut ByteBuf<N> {
        &mut self.0
    }
}

impl<N: ArrayLength<u8>> Der<N> {
    /// Create a new `Der` structure that writes values to the given buffer
    pub fn new() -> Self {
        Der(ByteBuf::new())
    }

    // // equivalent of method in std::io::Write
    // fn write_all(&mut self, data: &[u8]) -> Result {
    //     self.0.extend_from_slice(data)
    // }

    /// Return underlying buffer
    pub fn into_inner(self) -> ByteBuf<N> {
        self.0
    }

    // https://docs.microsoft.com/en-us/windows/win32/seccertenroll/about-encoded-length-and-value-bytes
    fn write_length_field(&mut self, length: usize) -> Result {
        if length < 0x80 {
            // values under 128: write length directly as u8
            self.extend_from_slice(&[length as u8])
        } else {
            // values at least 128:
            // - write number of bytes needed as u8, setting bit 7
            // - write l as big-endian bytes representation, with minimal length

            let mut repr = &length.to_be_bytes()[..];
            while repr[0] == 0 {
                repr = &repr[1..];
            }
            self.extend_from_slice(&[0x80 | repr.len() as u8])?;
            self.extend_from_slice(repr)
        }
    }

    //    // /// Write a `NULL` tag.
    //    // pub fn null(&mut self) -> Result {
    //    //     self.0.extend_from_slice(&[Tag::Null as u8, 0])?;
    //    //     Ok(())
    //    // }

    // /// Write an arbitrary tag-length-value
    // pub fn raw_tlv(&mut self, tag: Tag, value: &[u8]) -> Result {
    //     self.extend_from_slice(&[tag as u8])?;
    //     self.write_length_field(value.len())?;
    //     self.extend_from_slice(value)
    // }

    /// Write an arbitrary tag-length-value
    pub fn raw_tlv(&mut self, tag: u8, value: &[u8]) -> Result {
        self.extend_from_slice(&[tag])?;
        self.write_length_field(value.len())?;
        self.extend_from_slice(value)
    }

    /// Write an arbitrary tag-length-value with 2-byte tag
    /// NB: everything in ISO 7816 is big-endian
    pub fn raw_tlv2(&mut self, tag: u16, value: &[u8]) -> Result {
        self.extend_from_slice(&tag.to_be_bytes())?;
        self.write_length_field(value.len())?;
        self.extend_from_slice(value)
    }

    ///// Write the given input as integer.
    /////
    ///// Assumes `input` is the big-endian representation of a non-negative `Integer`
    /////
    ///// Not sure about good references, maybe:
    ///// https://docs.microsoft.com/en-us/windows/win32/seccertenroll/about-integer
    /////
    ///// From: https://docs.rs/ecdsa/0.3.0/src/ecdsa/convert.rs.html#205-219
    ///// Compute ASN.1 DER encoded length for the provided scalar.
    ///// The ASN.1 encoding is signed, so its leading bit must have value 0;
    ///// it must also be of minimal length (so leading bytes of value 0 must be
    ///// removed, except if that would contradict the rule about the sign bit).
    //pub fn non_negative_integer(&mut self, mut integer: &[u8]) -> Result {
    //    self.extend_from_slice(&[Tag::Integer as u8])?;

    //    // strip leading zero bytes
    //    while !integer.is_empty() && integer[0] == 0 {
    //        integer = &integer[1..];
    //    }

    //    if integer.is_empty() || integer[0] >= 0x80 {
    //        self.write_length_field(integer.len() + 1)?;
    //        self.extend_from_slice(&[0x00])?;
    //    } else {
    //        self.write_length_field(integer.len())?;
    //    }

    //    self.extend_from_slice(integer)
    //}

    /// Write a nested structure by passing in a handling function that writes
    /// the serialized intermediate structure.
    pub fn nested<F>(&mut self, tag: u8, f: F) -> Result
    where
        F: FnOnce(&mut Der<N>) -> Result,
    {
        let before = self.len();

        // serialize the nested structure
        f(self)?;
        let written = self.len() - before;

        // generate Tag-Length prefix
        // 1 for tag, 1 for length prefix, 4 or 8 for usize itself
        //
        // could try something like: type PrefixSize =<consts::U2 as core::ops::Add<consts::U8>>::Output;
        // but not couldn't find a consts::Usize type;
        type PrefixSize = consts::U12;
        let mut prefix = Der::<PrefixSize>::new();

        // generate prefix consisting of "tag" and length of nested structure
        prefix.extend_from_slice(&[tag])?;
        prefix.write_length_field(written)?;

        self.insert_slice_at(&prefix, before)
    }

    /// Write a `SEQUENCE` by passing in a handling function that writes to an intermediate `Vec`
    /// before writing the whole sequence to `self`.
    pub fn sequence<F>(&mut self, f: F) -> Result
    where
        F: FnOnce(&mut Der<N>) -> Result,
    {
        self.nested(Tag::Sequence as u8, f)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    //    #[test]
    //    fn max_prefix() {
    //        let mut u32_buf = [0u8; core::mem::size_of::<u32>() + 2];
    //        let mut prefix = Der::new(&mut u32_buf);
    //        prefix.0.extend_from_slice(&[0u8]).unwrap();
    //        assert!(prefix.write_length_field(u32::max_value() as usize).is_ok());
    //        assert_eq!([0u8, 132, 255, 255, 255, 255], prefix.as_ref());

    //        let mut u64_buf = [0u8; core::mem::size_of::<u64>() + 2];
    //        let mut prefix = Der::new(&mut u64_buf);
    //        prefix.0.extend_from_slice(&[0u8]).unwrap();
    //        assert!(prefix.write_length_field(u64::max_value() as usize).is_ok());
    //        assert_eq!([0, 136, 255, 255, 255, 255, 255, 255, 255, 255], prefix.as_ref());
    //    }

    #[test]
    fn write_asn1_der_ecdsa_signature() {
        let r = [
            167u8, 156, 58, 251, 253, 197, 176, 208, 165, 146, 155, 16, 217, 152, 192, 243, 206,
            76, 214, 207, 207, 180, 237, 8, 156, 160, 64, 32, 147, 82, 213, 158,
        ];
        let s = [
            184, 156, 136, 100, 87, 142, 84, 61, 235, 27, 193, 223, 254, 97, 11, 111, 80, 37, 46,
            150, 121, 96, 165, 96, 65, 242, 211, 180, 175, 91, 158, 88,
        ];
        // let mut buf = [0u8; 1024];
        let mut der = Der::<consts::U1024>::new();
        der.sequence(|der| {
            der.non_negative_integer(&r)?;
            der.non_negative_integer(&s)
        })
        .unwrap();

        #[rustfmt::skip]
        let expected = [
            48u8, 70,
            2, 33,
                0, 167, 156, 58, 251, 253, 197, 176, 208, 165, 146, 155, 16, 217, 152,
                192, 243, 206, 76, 214, 207, 207, 180, 237, 8, 156, 160, 64, 32, 147, 82, 213, 158,
            2, 33,
                0, 184, 156, 136, 100, 87, 142, 84, 61, 235, 27, 193, 223, 254, 97, 11, 111, 80,
                37, 46, 150, 121, 96, 165, 96, 65, 242, 211, 180, 175, 91, 158, 88,
        ];
        assert_eq!(der.len(), expected.len());
        assert_eq!(
            ByteBuf::<consts::U72>::from_slice(&der).unwrap(),
            ByteBuf::<consts::U72>::from_slice(&expected).unwrap(),
        );
        // assert_eq!(&got[..32], &expected[..32]);
        // assert_eq!(&got[32..64], &expected[32..64]);
        // assert_eq!(&got[64..], &expected[64..]);
    }
}

//// let mut der = Der::new(&mut buf);
//// der.sequence(|der| {
////     der.positive_integer(n)?;
////     der.positive_integer(e)
//// })
//// .unwrap();

//// /// Write an `OBJECT IDENTIFIER`.
//// pub fn oid(&mut self, input: &[u8]) -> Result<()> {
////     self.writer.0.extend_from_slice(&[Tag::Oid as u8])?;
////     self.write_length_field(input.len())?;
////     self.writer.0.extend_from_slice(&input)?;
////     Ok(())
//// }

//// /// Write raw bytes to `self`. This does not calculate length or apply. This should only be used
//// /// when you know you are dealing with bytes that are already DER encoded.
//// pub fn raw(&mut self, input: &[u8]) -> Result<()> {
////     Ok(self.writer.0.extend_from_slice(input)?)
//// }

//// /// Write a `BIT STRING`.
//// pub fn bit_string(&mut self, unused_bits: u8, bit_string: &[u8]) -> Result<()> {
////     self.writer.0.extend_from_slice(&[Tag::BitString as u8])?;
////     self.write_length_field(bit_string.len() + 1)?;
////     self.writer.0.extend_from_slice(&[unused_bits])?;
////     self.writer.0.extend_from_slice(&bit_string)?;
////     Ok(())
//// }

//// /// Write an `OCTET STRING`.
//// pub fn octet_string(&mut self, octet_string: &[u8]) -> Result<()> {
////     self.writer.0.extend_from_slice(&[Tag::OctetString as u8])?;
////     self.write_length_field(octet_string.len())?;
////     self.writer.0.extend_from_slice(&octet_string)?;
////     Ok(())
//// }
//// }

//// #[cfg(test)]
//// mod test {
////     use super::*;
////     use untrusted::Input;
////     use Error;

////     static RSA_2048_PKCS1: &'static [u8] = include_bytes!("../tests/rsa-2048.pkcs1.der");

////     #[test]
////     fn write_pkcs1() {
////         let input = Input::from(RSA_2048_PKCS1);
////         let (n, e) = input
////             .read_all(Error::Read, |input| {
////                 der::nested(input, Tag::Sequence, |input| {
////                     let n = der::positive_integer(input)?;
////                     let e = der::positive_integer(input)?;
////                     Ok((n.as_slice_less_safe(), e.as_slice_less_safe()))
////                 })
////             })
////             .unwrap();

////         let mut buf = Vec::new();
////         {
////             let mut der = Der::new(&mut buf);
////             der.sequence(|der| {
////                 der.positive_integer(n)?;
////                 der.positive_integer(e)
////             })
////             .unwrap();
////         }

////         assert_eq!(buf.as_slice(), RSA_2048_PKCS1);
////     }

////     #[test]
////     fn write_octet_string() {
////         let mut buf = Vec::new();
////         {
////             let mut der = Der::new(&mut buf);
////             der.octet_string(&[]).unwrap();
////         }

////         assert_eq!(&buf, &[0x04, 0x00]);

////         let mut buf = Vec::new();
////         {
////             let mut der = Der::new(&mut buf);
////             der.octet_string(&[0x0a, 0x0b, 0x0c]).unwrap();
////         }

////         assert_eq!(&buf, &[0x04, 0x03, 0x0a, 0x0b, 0x0c]);
////     }
//// }
