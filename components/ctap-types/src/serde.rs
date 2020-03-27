pub mod de;
pub mod ser;
pub mod error;

pub use error::{Error, Result};

// pub use de::from_bytes;
// pub use de::take_from_bytes;

// TODO: reimplement here
pub fn cbor_serialize_old<T: serde::Serialize>(
    object: &T,
    buffer: &mut [u8],
) -> core::result::Result<usize, serde_cbor::Error> {
    let writer = serde_cbor::ser::SliceWrite::new(buffer);
    let mut ser = serde_cbor::Serializer::new(writer);

    object.serialize(&mut ser)?;

    let writer = ser.into_inner();
    let size = writer.bytes_written();

    Ok(size)
}

pub fn cbor_serialize<T: serde::Serialize>(
    object: &T,
    buffer: &mut [u8],
) -> Result<usize> {
    let writer = ser::SliceWriter::new(buffer);
    let mut ser = ser::Serializer::new(writer);

    object.serialize(&mut ser)?;

    let writer = ser.into_inner();
    let size = writer.bytes_written();

    Ok(size)
}


pub fn cbor_deserialize<'de, T: serde::Deserialize<'de>>(
    buffer: &'de [u8],
) -> Result<T> {
    de::from_bytes(buffer)
}

