pub mod de;
pub mod ser;
pub mod error;

pub use error::{Error, Result};

// pub use de::from_bytes;
// pub use de::take_from_bytes;

// kudos to postcard, this is much nicer than returning size
pub fn cbor_serialize<'a, 'b, T: serde::Serialize>(
    object: &'a T,
    buffer: &'b mut [u8],
) -> Result<&'b [u8]> {
    let writer = ser::SliceWriter::new(buffer);
    let mut ser = ser::Serializer::new(writer);

    object.serialize(&mut ser)?;

    let writer = ser.into_inner();
    let size = writer.bytes_written();

    Ok(&buffer[..size])
}


pub fn cbor_serialize_bytes<'a, 'b, N: heapless_bytes::ArrayLength<u8>, T: serde::Serialize>(
    object: &'a T,
    bytes: &'b mut heapless_bytes::Bytes<N>,
) -> Result<usize> {
    let len_before = bytes.len();
    let mut ser = ser::Serializer::new(bytes);

    object.serialize(&mut ser)?;

    Ok(ser.into_inner().len() - len_before)
}


pub fn cbor_deserialize<'de, T: serde::Deserialize<'de>>(
    buffer: &'de [u8],
) -> Result<T> {
    // cortex_m_semihosting::hprintln!("deserializing {:?}", buffer).ok();
    de::from_bytes(buffer)
}

