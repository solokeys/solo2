


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


#[allow(non_camel_case_types)]
type U3072 = <
    heapless::consts::U2048 as core::ops::Add<heapless::consts::U1024>
 >::Output;

// The max size that APDU dispatch will buffer chained commands up to.
 #[allow(non_camel_case_types)]
pub(crate) type LARGE_APDU_SIZE = U7609;

// The max size that will be "transmitted" through the interchanges
 #[allow(non_camel_case_types)]
pub(crate) type MEDIUM_APDU_SIZE = U3072;


pub mod command {
    use super::*;
    pub type Size = LARGE_APDU_SIZE;
    pub type Data = iso7816::Bytes<Size>;
}

pub mod response {
    use super::*;
    pub type Size = LARGE_APDU_SIZE;
    pub type Data = iso7816::Bytes<Size>;
}

pub mod interchanges {
    use super::*;
    pub type Size = MEDIUM_APDU_SIZE;
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


