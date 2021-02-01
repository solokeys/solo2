pub mod status;
pub use status::Status;

pub type Data = crate::Bytes<crate::MAX_COMMAND_DATA>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response {
    Data(Data),
    Status(Status),
}

impl Default for Response {
    fn default() -> Self {
        Self::Status(Default::default())
    }
}

pub type Result = core::result::Result<Data, Status>;

impl From<Result> for Response {
    fn from(result: Result) -> Self {
        match result {
            Ok(data) => Self::Data(data),
            Err(status) => Self::Status(status)
        }
    }
}

impl Into<Result> for Response {
    fn into(self) -> Result {
        match self {
            Self::Data(data) => Ok(data),
            Self::Status(status) => Err(status),
        }
    }
}

// #[derive(Clone, Debug, Default, Eq, PartialEq)]
// pub struct Response {
//     pub status: Status,
//     pub data: Data,
// }

// impl From<Result> for Response {
//     fn from(result: Result) -> Self {
//         match result {
//             Ok(data) => {
//                 Response {
//                     status: Default::default(),
//                     data,
//                 }
//             }
//             Err(status) => {
//                 Response {
//                     status,
//                     data: Default::default(),
//                 }
//             }
//         }
//     }
// }

impl Response {
    pub fn into_message(&self) -> Data {
        let mut message = Data::new();
        let status = match self {
            Self::Data(data) => {
                message.extend_from_slice(&data).unwrap();
                Status::default()
            }
            Self::Status(status) => *status,
        };

        let status_bytes: [u8; 2] = status.into();
        message.extend_from_slice(&status_bytes).unwrap();
        message
    }
}
