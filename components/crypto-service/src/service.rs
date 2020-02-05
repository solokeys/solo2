use crate::api::*;
use crate::config::*;
use crate::error::Error;
use crate::types::*;

pub use crate::pipe::ServiceEndpoint;
pub use embedded_hal::blocking::rng::Read as RngRead;

// associated keys end up namespaced under "/fido2"
// example: "/fido2/keys/2347234"
// let (mut fido_endpoint, mut fido2_client) = Client::new("fido2");
// let (mut piv_endpoint, mut piv_client) = Client::new("piv");

pub struct Service<'a, 's, Rng, PersistentStorage, VolatileStorage>
where
    Rng: RngRead,
    PersistentStorage: LfsStorage,
    VolatileStorage: LfsStorage,
{
    eps: Vec<ServiceEndpoint<'a>, MAX_SERVICE_CLIENTS>,
    rng: Rng,
    // maybe make this more flexible later, but not right now
    pfs: FilesystemWith<'s, 's, PersistentStorage>,
    vfs: FilesystemWith<'s, 's, VolatileStorage>,
}

// PANICS
const HEX_CHARS: &[u8] = b"0123456789abcdef";
fn format_hex(data: &[u8], mut buffer: &mut [u8]) {
    for byte in data.iter() {
        buffer[0] = HEX_CHARS[(byte >> 4) as usize];
        buffer[1] = HEX_CHARS[(byte & 0xf) as usize];
        buffer = &mut buffer[2..];
    }
}

impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> Service<'a, 's, R, P, V> {

    pub fn new(
        rng: R,
        persistent_storage: FilesystemWith<'s, 's, P>,
        volatile_storage: FilesystemWith<'s, 's, V>,
    )
        -> Self
    {
        Self {
            eps: Vec::new(),
            rng,
            pfs: persistent_storage,
            vfs: volatile_storage,
        }
    }

    pub fn add_endpoint(&mut self, ep: ServiceEndpoint<'a>) -> Result<(), ServiceEndpoint> {
        self.eps.push(ep)
    }

    // process one request per client which has any
    pub fn process(&mut self) {
        // pop request in channel
        for ep in self.eps.iter_mut() {
            if !ep.send.ready() {
                continue;
            }
            if let Some(request) = ep.recv.dequeue() {
                #[cfg(test)]
                println!("service got request: {:?}", &request);

                match request {
                    Request::DummyRequest => {
                        #[cfg(test)]
                        println!("got a dummy request!");
                        ep.send.enqueue(Ok(Reply::DummyReply)).ok();
                    },

                    // TODO: use the `?` operator
                    // TODO: how to handle queue failure?
                    // TODO: decouple this in such a way that we can easily extend the
                    //       cryptographic capabilities on two axes:
                    //        - mechanisms
                    //        - backends
                    Request::GenerateKeypair(request) => {
                        match request.mechanism {
                            Mechanism::Ed25519 => {

                                // generate key
                                let mut seed = [0u8; 32];
                                if self.rng.read(&mut seed).is_err() {
                                    ep.send.enqueue(Err(Error::EntropyMalfunction)).ok();
                                    return;
                                }

                                // not needed now.
                                // do we want to cache its public key?
                                //
                                // let keypair = salty::Keypair::from(&seed);
                                // #[cfg(all(test, feature = "verbose-tests"))]
                                // println!("ed25519 keypair with public key = {:?}", &keypair.public);

                                // generate unique id
                                let mut unique_id = [0u8; 16];
                                if self.rng.read(&mut unique_id).is_err() {
                                    ep.send.enqueue(Err(Error::EntropyMalfunction)).ok();
                                    return;
                                }
                                #[cfg(all(test, feature = "verbose-tests"))]
                                println!("unique id {:?}", &unique_id);

                                // store key
                                // TODO: add "app" namespacing, and AEAD this ID
                                // let mut path = [0u8; 38];
                                // path[..6].copy_from_slice(b"/test/");
                                // format_hex(&unique_id, &mut path[6..]);
                                let mut path = [0u8; 33];
                                path[..1].copy_from_slice(b"/");
                                format_hex(&unique_id, &mut path[1..]);
                                #[cfg(test)]
                                println!("storing in file {:?}", unsafe { core::str::from_utf8_unchecked(&path[..]) });

                                use littlefs2::fs::{File, FileWith};
                                let mut alloc = File::allocate();
                                // TODO: fail request on filesystem malfunction
                                let mut file = match FileWith::create(&path[..], &mut alloc, &mut self.vfs) {
                                    Ok(file) => file,
                                    Err(_) => {
                                        ep.send.enqueue(Err(Error::FilesystemWriteFailure)).ok();
                                        return;
                                    }
                                };
                                use littlefs2::io::WriteWith;
                                if file.write(&seed).is_err() {
                                    ep.send.enqueue(Err(Error::FilesystemWriteFailure)).ok();
                                    return;
                                }
                                if file.sync().is_err() {
                                    ep.send.enqueue(Err(Error::FilesystemWriteFailure)).ok();
                                    return;
                                }

                                // return key handle
                                ep.send.enqueue(Ok(Reply::GenerateKey(
                                    reply::GenerateKey { key_handle: KeyHandle { key_id: unique_id } }))).unwrap();
                            },

                            #[allow(unreachable_patterns)]
                            _ => {
                                ep.send.enqueue(Err(Error::MechanismNotAvailable)).ok();
                            }
                        }
                    },
                    _ => {
                        #[cfg(test)]
                        println!("todo: {:?} request!", &request);
                        ep.send.enqueue(Err(Error::RequestNotAvailable)).ok();
                    },
                }
            }
        }
    }
}

