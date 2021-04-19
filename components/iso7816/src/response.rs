use heapless_bytes::ArrayLength;

pub mod status;
pub use status::Status;

pub type Data<T> = crate::Bytes<T>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response<SIZE>
where SIZE: ArrayLength<u8>
{
    Data(Data<SIZE>),
    Status(Status),
}

impl<SIZE> Default for Response<SIZE>
where SIZE: ArrayLength<u8> {
    fn default() -> Self {
        Self::Status(Default::default())
    }
}
