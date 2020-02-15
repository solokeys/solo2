#![cfg(test)]

//! Due to our use of global pipes, in case of failing tests run with:
//! `cargo test -- --test-threads 1 --nocapture`

use core::task::Poll;

use chacha20::ChaCha20;
use littlefs2::ram_storage;

use crate::*;
use crate::types::*;

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

static mut REQUEST_PIPE: pipe::RequestPipe = heapless::spsc::Queue(heapless::i::Queue::u8());
static mut REPLY_PIPE: pipe::ReplyPipe = heapless::spsc::Queue(heapless::i::Queue::u8());

struct MockRng(ChaCha20);

impl MockRng {
    pub fn new() -> Self {
		use chacha20::stream_cipher::generic_array::GenericArray;
		use chacha20::stream_cipher::NewStreamCipher;
        let key = GenericArray::from_slice(b"an example very very secret key.");
        let nonce = GenericArray::from_slice(b"secret nonce");
        Self(ChaCha20::new(&key, &nonce))
    }
}

impl crate::service::RngRead for MockRng {
    type Error = core::convert::Infallible;

    fn read(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
		use chacha20::stream_cipher::SyncStreamCipher;
        self.0.apply_keystream(buf);
        Ok(())
    }
}

ram_storage!(PersistentStorage, PersistentRam, 4096);
ram_storage!(VolatileStorage, VolatileRam, 4096);

// hmm how to export variable?
// macro_rules! setup_storage {
//     () => {
//         // need to figure out if/how to do this as `static mut`
//         let mut persistent_ram = PersistentRam::default();
//         let mut persistent_storage = PersistentStorage::new(&mut persistent_ram);
//         Filesystem::format(&mut persistent_storage).expect("could not format persistent storage");
//         let mut persistent_fs_alloc = Filesystem::allocate();
//         let mut pfs = Filesystem::mount(&mut persistent_fs_alloc, &mut persistent_storage)
//                 .expect("could not mount persistent storage");

//         let mut volatile_ram = VolatileRam::default();
//         let mut volatile_storage = VolatileStorage::new(&mut volatile_ram);
//         Filesystem::format(&mut volatile_storage).expect("could not format volatile storage");
//         let mut volatile_fs_alloc = Filesystem::allocate();
//         let mut vfs = Filesystem::mount(&mut volatile_fs_alloc, &mut volatile_storage)
//                 .expect("could not mount volatile storage");
//     }
// }

#[test]
fn dummy() {
    use heapless::spsc::Queue;

    // local setup:
    // let mut request_pipe = pipe::RequestPipe::u8();
    // let mut reply_pipe = pipe::ReplyPipe::u8();
    // let (service_endpoint, client_endpoint) =
    //     pipe::new_endpoints(&mut request_pipe, &mut reply_pipe);

    // static setup
    let (service_endpoint, client_endpoint) = pipe::new_endpoints(
        unsafe { &mut REQUEST_PIPE },
        unsafe { &mut REPLY_PIPE },
        "fido2",
    );

    let rng = MockRng::new();

    // setup_storage!();
    // need to figure out if/how to do this as `static mut`
    let mut persistent_ram = PersistentRam::default();
    let mut persistent_storage = PersistentStorage::new(&mut persistent_ram);
    Filesystem::format(&mut persistent_storage).expect("could not format persistent storage");
    let mut persistent_fs_alloc = Filesystem::allocate();
    let pfs = FilesystemWith::mount(&mut persistent_fs_alloc, &mut persistent_storage)
        .expect("could not mount persistent storage");

    let mut volatile_ram = VolatileRam::default();
    let mut volatile_storage = VolatileStorage::new(&mut volatile_ram);
    Filesystem::format(&mut volatile_storage).expect("could not format volatile storage");
    let mut volatile_fs_alloc = Filesystem::allocate();
    let vfs = FilesystemWith::mount(&mut volatile_fs_alloc, &mut volatile_storage)
        .expect("could not mount volatile storage");

    let mut service = Service::new(rng, pfs, vfs).expect("service init worked");
    assert!(service.add_endpoint(service_endpoint).is_ok());

    let mut client = RawClient::new(client_endpoint);

    // client gets injected into "app"
    // may perform crypto request at any time
    let mut future = client
        .request(crate::api::Request::DummyRequest)
        .map_err(drop)
        .unwrap();

    // service is assumed to be running in other thread
    // actually, the "request" method should pend an interrupt,
    // and said other thread should have higher priority.
    service.process();

    // this would likely be a no-op due to higher priority of crypto thread
    let reply = block!(future);

    assert_eq!(reply, Ok(Reply::DummyReply));
}

// #[test]
// fn sign_ed25519_raw() {
//     let (service_endpoint, client_endpoint) = pipe::new_endpoints(
//         unsafe { &mut REQUEST_PIPE },
//         unsafe { &mut REPLY_PIPE },
//         "fido2",
//     );

//     let rng = MockRng::new();

//     // need to figure out if/how to do this as `static mut`
//     let mut persistent_ram = PersistentRam::default();
//     let mut persistent_storage = PersistentStorage::new(&mut persistent_ram);
//     Filesystem::format(&mut persistent_storage).expect("could not format persistent storage");
//     let mut persistent_fs_alloc = Filesystem::allocate();
//     let pfs = FilesystemWith::mount(&mut persistent_fs_alloc, &mut persistent_storage)
//         .expect("could not mount persistent storage");
//     let mut volatile_ram = VolatileRam::default();
//     let mut volatile_storage = VolatileStorage::new(&mut volatile_ram);
//     Filesystem::format(&mut volatile_storage).expect("could not format volatile storage");
//     let mut volatile_fs_alloc = Filesystem::allocate();
//     let vfs = FilesystemWith::mount(&mut volatile_fs_alloc, &mut volatile_storage)
//         .expect("could not mount volatile storage");

//     let mut service = Service::new(rng, pfs, vfs);
//     service.add_endpoint(service_endpoint).ok();
//     let mut client = RawClient::new(client_endpoint);

//     // client gets injected into "app"
//     // may perform crypto request at any time
//     let request = api::request::GenerateKeypair {
//         mechanism: Mechanism::Ed25519,
//         key_attributes: types::KeyAttributes::default(),
//     };
//     // let mut future = client.request(request);
//     use crate::client::SubmitRequest;
//     let mut future = request
//         .submit(&mut client)
//         .map_err(drop)
//         .unwrap();

//     // service is assumed to be running in other thread
//     // actually, the "request" method should pend an interrupt,
//     // and said other thread should have higher priority.
//     service.process();

//     // this would likely be a no-op due to higher priority of crypto thread
//     let reply = block!(future);

//     let keypair_handle = if let Ok(Reply::GenerateKeypair(actual_reply)) = reply {
//         actual_reply.keypair_handle
//     } else {
//         panic!("unexpected reply {:?}", reply);
//     };

//     // local = generated on device, or copy of such
//     // (what about derived from local key via HKDF? pkcs#11 says no)

//     let message = [1u8, 2u8, 3u8];
//     // let signature = fido2_client.keypair.sign(&mut context, &message);
//     let request = api::request::Sign {
//         key_handle: keypair_handle,
//         mechanism: Mechanism::Ed25519,
//         message: Message::try_from_slice(&message).expect("all good"),
//     };

//     let mut future = request.submit(&mut client).map_err(drop).unwrap();
//     service.process();
//     let reply = block!(future);
// }

#[test]
fn sign_ed25519() {
    let (service_endpoint, client_endpoint) = pipe::new_endpoints(
        unsafe { &mut REQUEST_PIPE },
        unsafe { &mut REPLY_PIPE },
        "fido2",
    );

    let rng = MockRng::new();

    // need to figure out if/how to do this as `static mut`
    let mut persistent_ram = PersistentRam::default();
    let mut persistent_storage = PersistentStorage::new(&mut persistent_ram);
    Filesystem::format(&mut persistent_storage).expect("could not format persistent storage");
    let mut persistent_fs_alloc = Filesystem::allocate();
    let pfs = FilesystemWith::mount(&mut persistent_fs_alloc, &mut persistent_storage)
        .expect("could not mount persistent storage");
    let mut volatile_ram = VolatileRam::default();
    let mut volatile_storage = VolatileStorage::new(&mut volatile_ram);
    Filesystem::format(&mut volatile_storage).expect("could not format volatile storage");
    let mut volatile_fs_alloc = Filesystem::allocate();
    let vfs = FilesystemWith::mount(&mut volatile_fs_alloc, &mut volatile_storage)
        .expect("could not mount volatile storage");

    let mut service = Service::new(rng, pfs, vfs).expect("service init worked");
    service.add_endpoint(service_endpoint).ok();

    // Client needs a "Syscall" trait impl, to trigger crypto processing
    // For testing, we use "self service",
    // meaning `&mut service` itself with the trivial implementation
    let syscaller = &mut service;
    let mut client = Client::new(client_endpoint, syscaller);

    let mut future = client.generate_ed25519_keypair().expect("no client error");
    println!("submitted gen ed25519");
    let reply = block!(future);
    let handles = reply.expect("no errors, never");
    let private_key = handles.private_key;
    let public_key = handles.public_key;
    println!("got a private {:?}, public {:?}", &private_key, &public_key);

    let message = [1u8, 2u8, 3u8];
    let mut future = client.sign_ed25519(&private_key, &message).expect("no client error");
    let reply: Result<api::reply::Sign, _> = block!(future);
    let signature = reply.expect("good signature").signature;
    println!("got a signature: {:?}", &signature);

    let mut future = client.verify_ed25519(&public_key, &message, &signature).expect("no client error");
    let reply = block!(future);
    let valid = reply.expect("good signature").valid;
    // assert!(valid);

    let mut future = client.verify_ed25519(&public_key, &message, &[1u8,2,3]).expect("no client error");
    let reply = block!(future);
    assert_eq!(Err(Error::WrongSignatureLength), reply);
}

#[test]
fn sign_p256() {
    let (service_endpoint, client_endpoint) = pipe::new_endpoints(
        unsafe { &mut REQUEST_PIPE },
        unsafe { &mut REPLY_PIPE },
        "fido2",
    );

    let rng = MockRng::new();

    // need to figure out if/how to do this as `static mut`
    let mut persistent_ram = PersistentRam::default();
    let mut persistent_storage = PersistentStorage::new(&mut persistent_ram);
    Filesystem::format(&mut persistent_storage).expect("could not format persistent storage");
    let mut persistent_fs_alloc = Filesystem::allocate();
    let pfs = FilesystemWith::mount(&mut persistent_fs_alloc, &mut persistent_storage)
        .expect("could not mount persistent storage");
    let mut volatile_ram = VolatileRam::default();
    let mut volatile_storage = VolatileStorage::new(&mut volatile_ram);
    Filesystem::format(&mut volatile_storage).expect("could not format volatile storage");
    let mut volatile_fs_alloc = Filesystem::allocate();
    let vfs = FilesystemWith::mount(&mut volatile_fs_alloc, &mut volatile_storage)
        .expect("could not mount volatile storage");

    let mut service = Service::new(rng, pfs, vfs).expect("service init worked");
    service.add_endpoint(service_endpoint).ok();

    // Client needs a "Syscall" trait impl, to trigger crypto processing
    // For testing, we use "self service",
    // meaning `&mut service` itself with the trivial implementation
    let syscaller = &mut service;
    let mut client = Client::new(client_endpoint, syscaller);



    let keys = block!(client.generate_p256_keypair().expect("no client error"))
        .expect("no errors");

    let message = [1u8, 2u8, 3u8];
    let signature = block!( client.sign_p256_prehashed(&keys.private_key, &message).expect("no client error"))
        .expect("good signature")
        .signature;

    let future = client.verify_p256_prehashed(&keys.public_key, &message, &signature);
    let mut future = future.expect("no client error");
    let result = block!(future);
    if result.is_err() {
        println!("error: {:?}", result);
    }
    let reply = result.expect("valid signature");
    let valid = reply.valid;
    assert!(valid);

}

