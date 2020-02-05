// use core::task::Poll;

// use crate::api::Reply;
// use crate::client::RawClient;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
    EntropyMalfunction,
    FilesystemWriteFailure,
    MechanismNotAvailable,
    RequestNotAvailable,
}

// pub struct FutureResult<'a, 'c> {
//     c: &'c mut RawClient<'a>,
// }

// impl<'a, 'c> FutureResult<'a, 'c> {
//     pub fn new(client: &'c mut RawClient<'a>) -> Self {
//         Self { c: client }
//     }

//     pub fn poll(&mut self) -> Poll<core::result::Result<Reply, Error>> {
//         // pop request in channel
//         match self.c.ep.recv.dequeue() {
//             Some(reply) => {
//                 #[cfg(all(test, feature = "verbose-tests"))]
//                 println!("got a reply");
//                 Poll::Ready(reply)
//             },
//             _ => Poll::Pending
//         }
//     }
// }


