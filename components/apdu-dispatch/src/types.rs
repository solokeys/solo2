use heapless_bytes::Unsigned;


#[allow(non_camel_case_types)]
type U6144 = <
    heapless::consts::U4096 as core::ops::Add<heapless::consts::U2048>
 >::Output;

type U7168 = <
    U6144 as core::ops::Add<heapless::consts::U1024>
 >::Output;

pub type U7609 = <
    U7168 as core::ops::Add<heapless::consts::U441>
 >::Output;

type U3072 = <
    heapless::consts::U2048 as core::ops::Add<heapless::consts::U1024>
 >::Output;


pub mod command {
    use super::*;
    pub type Size = U7609;
    pub const SIZE: usize = Size::USIZE;
    pub type Data = iso7816::Bytes<Size>;
}

pub mod response {
    use super::*;
    pub type Size = U7609;
    pub const SIZE: usize = Size::USIZE;
    pub type Data = iso7816::Bytes<Size>;
}

pub mod interchanges {
    use super::*;
    pub type Size = U3072;
    pub const SIZE: usize = Size::USIZE;
    pub type Data = iso7816::Bytes<Size>;

    interchange::interchange! {
        Contact: (Data, Data)
    }

    interchange::interchange! {
        Contactless: (Data, Data)
    }
}


// What apps can expect to send and recieve.
pub type Command = iso7816::Command<command::Size>;
pub type Response = iso7816::Response<response::Size>;

