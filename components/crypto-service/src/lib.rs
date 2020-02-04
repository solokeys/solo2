#![cfg_attr(not(test), no_std)]
use heapless::{
    consts::{
        self,
        U1,
        U2,
    },
    spsc::{
        Consumer,
        Producer,
        Queue,
    },
    Vec,
};

use heapless_bytes::Bytes;

#[allow(non_camel_case_types)]
pub type MAX_MESSAGE_LENGTH = consts::U1024;
#[allow(non_camel_case_types)]
pub type MAX_SIGNATURE_LENGTH = consts::U72;

type KeyId = u32;

/// Opaque key handle
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct KeyHandle{
    key_id: KeyId,
}

impl KeyHandle {
    pub fn unique_id(&self) -> KeyId {
        self.key_id
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Mechanism {
    Ed25519,
    // P256,
    // X25519,
}

// for counters use the pkcs#11 idea of
// a monotonic incrementing counter that
// "increments on each read" --> save +=1 operation

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GenerateKeyRequest {
    mechanism: Mechanism,
    // key_template: KeyTemplate,
}

pub struct GenerateKeyReply {
    key_handle: KeyHandle,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GenerateKeypairRequest {
    mechanism: Mechanism,
    // private_key_template: PrivateKeyTemplate,
    // public_key_template: PublicKeyTemplate,
}

pub struct GenerateKeypairReply {
    public_key_handle: KeyHandle,
    private_key_handle: KeyHandle,
}

pub type Message = Bytes<MAX_MESSAGE_LENGTH>;
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SignRequest {
    key_handle: KeyHandle,
    mechanism: Mechanism,
    message: Message,
}

pub type Signature = Bytes<MAX_SIGNATURE_LENGTH>;
pub struct SignReply {
    signature: Signature,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Request {
    DummyRequest,
    GenerateKey(GenerateKeyRequest),
    GenerateKeypair(GenerateKeypairRequest),
    Sign(SignRequest),
}

// pub struct DummyReply {}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Reply {
    DummyReply,
    GenerateKeyReply,
    GenerateKeypairReply,
    SignReply,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
}

pub struct ServerEndpoint<'a> {
    pub recv: Consumer<'a, Request, U1, u8>,
    pub send: Producer<'a, Result<Reply, Error>, U1, u8>,
}

pub struct ClientEndpoint<'a> {
    pub recv: Consumer<'a, Result<Reply, Error>, U1, u8>,
    pub send: Producer<'a, Request, U1, u8>,
}

pub struct Server<'a> {
    eps: Vec<ServerEndpoint<'a>, U2>,
}

impl<'a> Server<'a> {
    pub fn new() -> Self {
        Self { eps: Vec::new() }
    }

    pub fn add_endpoint(&mut self, ep: ServerEndpoint<'a>) -> Result<(), ServerEndpoint> {
        self.eps.push(ep)
    }

    // process one client's request, if any
    pub fn process(&mut self) {
        // pop request in channel
        for ep in self.eps.iter_mut() {
            match ep.recv.dequeue() {
                Some(request) => {
                    #[cfg(test)]
                    println!("got a request!");
                    ep.send.enqueue(Ok(Reply::DummyReply));
                    return;
                },
                _ => {}
            }
        }
    }
}

pub struct RawClient<'a> {
    ep: ClientEndpoint<'a>,
}

use core::task::Poll;

pub struct FutureReplyResult<'a, 'c> {
    c: &'c mut RawClient<'a>,
}

impl FutureReplyResult<'_, '_> {
    pub fn poll(&mut self) -> Poll<Result<Reply, Error>> {
        // pop request in channel
        match self.c.ep.recv.dequeue() {
            Some(reply) => {
                #[cfg(test)]
                println!("got a reply");
                Poll::Ready(reply)
            },
            _ => Poll::Pending
        }
    }
}

impl<'a> RawClient<'a> {
    pub fn new(ep: ClientEndpoint<'a>) -> Self {
        Self { ep }
    }

    pub fn request<'c>(&'c mut self, req: Request) -> FutureReplyResult<'a, 'c> {
        self.ep.send.enqueue(req);
        FutureReplyResult {
            c: self,
        }
    }

    // pub fn reply(&mut self, ) -> Result<Reply> {
    //     None
    // }
}

pub struct Client {
}

impl Client {
    // pub fn sign(&mut self, key_handle: KeyHandle, mechanism: Mechanism, data: &[u8])
    //     -> core::task::Poll<Signature>
    // {
    //     todo!();
    // }

    // hmm this function signature
    // what i want is to temporarily borrow the receiving end of the
    // pipe to Crypty, and release it once the signature result arrives.
    //
    // Also note recent discussion on CFRG: do NOT pass in public key
    // separately (users can mix up, implementation detail of Crypty
    // whether to cache/store the public key with the private key as key
    // pair, or calculate public from private key on the fly).
    // pub fn sign_ed25519(&mut self, private_key: KeyHandle, data: &[u8])
    //     -> FutureResult<Signature>
    // {
    //     todo!();
    // }

    // generally, don't offer multiple versions of a mechanism, if possible.
    // try using the simplest when given the choice.
    // hashing is something users can do themselves hopefully :)
    // pub fn sign_p256_prehashed(&mut self, private_key: KeyHandle, data: &[u8])
    //     -> core::task::Poll<Signature>
    // {
    //     todo!();
    // }

}

// pub fn new_crypto_pipe(prefix: &'static str) -> (ServerEndpoint, ClientEndpoint) {
//     tr
// }


#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! block {
        ($future_result:expr) => {
            loop {
                match $future_result.poll() {
                    Poll::Ready(result) => { break result; },
                    Poll::Pending => {},
                }
            }
        }
    }

    #[test]
    fn sign() {
        let mut request_pipe = Queue::<Request, U1, u8>::u8();
        let (mut req_send, mut req_recv) = request_pipe.split();
        let mut reply_pipe = Queue::<Result<Reply, Error>, U1, u8>::u8();
        let (mut rep_send, mut rep_recv) = reply_pipe.split();

        let server_endpoint = ServerEndpoint { recv: req_recv, send: rep_send };
        let client_endpoint = ClientEndpoint { recv: rep_recv, send: req_send };

        // associated keys end up namespaced under "/fido2"
        // example: "/fido2/keys/2347234"
        // let (mut fido_endpoint, mut fido2_client) = Client::new("fido2");
        // let (mut piv_endpoint, mut piv_client) = Client::new("piv");

        let mut server = Server::new();
        assert!(server.add_endpoint(server_endpoint).ok().is_some());
        // server.add_endpoint(piv_endpoint).unwrap();

        // client gets injected into "app"
        let mut client = RawClient::new(client_endpoint);

        // may perform crypto request at any time
        let request = GenerateKeypairRequest { mechanism: Mechanism::Ed25519 };
        let mut future = client.request(Request::GenerateKeypair(request));

        // server is assumed to be running in other thread
        // actually, the "request" method should pend an interrupt,
        // and said other thread should have higher priority.
        server.process();

        // this would likely be a no-op due to higher priority of crypto thread
        let reply = block!(future);

        assert_eq!(reply, Ok(Reply::DummyReply));

        // let options = KeypairOptions {
        //     mach: AsymmetricAlgorithm::Ed25519,
        //     // never return naked private key
        //     sensitive: true,
        //     // do not even return wrapped private key
        //     extractable: false,
        //     // do not save to disk
        //     persistent: true,
        // };

        // local = generated on device, or copy of such
        // (what about derived from local key via HKDF? pkcs#11 says no)

        // let message = [1u8, 2u8, 3u8];
        // let signature = fido2_client.keypair.sign(&mut context, &message);

    }
}








