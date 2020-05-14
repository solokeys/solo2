use heapless::{
    consts::U1,
    spsc::{
        Consumer,
        Producer,
        Queue,
    },
};

use crate::api::{Request, Reply};
use crate::error::Error;
use crate::types::ClientId;

// TODO: The request pipe should block if there is an unhandled
// previous request/reply. As a side effect, the service should always
// be able to assume that the reply pipe is "ready".

// PRIOR ART:
// https://xenomai.org/documentation/xenomai-2.4/html/api/group__native__queue.html
// https://doc.micrium.com/display/osiiidoc/Using+Message+Queues

pub type RequestPipe = Queue::<Request, U1, u8>;
pub type ReplyPipe = Queue::<Result<Reply, Error>, U1, u8>;

pub /*unsafe*/ fn new_endpoints(
    request_pipe: &'static mut RequestPipe,
    reply_pipe: &'static mut ReplyPipe,
    client_id: ClientId,
    )
    -> (ServiceEndpoint, ClientEndpoint)
{
    let (req_send, req_recv) = request_pipe.split();
    let (rep_send, rep_recv) = reply_pipe.split();
    let service_endpoint = ServiceEndpoint { recv: req_recv, send: rep_send, client_id };
    let client_endpoint = ClientEndpoint { recv: rep_recv, send: req_send };
    (service_endpoint, client_endpoint)
}

pub struct ServiceEndpoint {
    pub recv: Consumer<'static, Request, U1, u8>,
    pub send: Producer<'static, Result<Reply, Error>, U1, u8>,
    // service (trusted) has this, not client (untrusted)
    // used among other things to namespace cryptographic material
    pub client_id: ClientId,
}

pub struct ClientEndpoint {
    pub recv: Consumer<'static, Result<Reply, Error>, U1, u8>,
    pub send: Producer<'static, Request, U1, u8>,
}

// in testing, this just directly calls service.process()
// in reality, this should rtfm::pend() the interrupt with handler triggering the service
pub trait Syscall {
    fn syscall(&mut self);
}

