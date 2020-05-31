#![cfg(test)]

//! Due to our use of global pipes, in case of failing tests run with:
//! `cargo test -- --test-threads 1 --nocapture`

use core::task::Poll;

use chacha20::ChaCha20;
use littlefs2::ram_storage;

use crate::*;
use crate::types::*;

struct MockRng(ChaCha20);

impl MockRng {
    pub fn new() -> Self {
		// use chacha20::stream_cipher::generic_array::GenericArray;
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

ram_storage!(InternalStorage, InternalRam, 4096);
ram_storage!(ExternalStorage, ExternalRam, 4096);
ram_storage!(VolatileStorage, VolatileRam, 4096);

fn raw_setup<F>(f: F)
where F: Fn(
    &mut Service::<'_, '_, tests::MockRng, tests::InternalStorage<'_>, tests::ExternalStorage<'_>, tests::VolatileStorage<'_>>,
    &mut crate::client::RawClient<'_>,
) {

    // whole lotta setup goin' on ;)

    let mut request_queue = heapless::spsc::Queue(heapless::i::Queue::u8());
    let mut reply_queue = heapless::spsc::Queue(heapless::i::Queue::u8());
    let (service_endpoint, client_endpoint) = pipe::new_endpoints(&mut request_queue, &mut reply_queue, "fido2");
    let rng = MockRng::new();

    let mut internal_ram = InternalRam::default();
    let mut internal_storage = InternalStorage::new(&mut internal_ram);
    Filesystem::format(&mut internal_storage).expect("could not format internal storage");
    let mut internal_fs_alloc = Filesystem::allocate();
    let ifs = FilesystemWith::mount(&mut internal_fs_alloc, &mut internal_storage)
        .expect("could not mount internal storage");

    let mut external_ram = ExternalRam::default();
    let mut external_storage = ExternalStorage::new(&mut external_ram);
    Filesystem::format(&mut external_storage).expect("could not format external storage");
    let mut external_fs_alloc = Filesystem::allocate();
    let efs = FilesystemWith::mount(&mut external_fs_alloc, &mut external_storage)
        .expect("could not mount external storage");

    let mut volatile_ram = VolatileRam::default();
    let mut volatile_storage = VolatileStorage::new(&mut volatile_ram);
    Filesystem::format(&mut volatile_storage).expect("could not format volatile storage");
    let mut volatile_fs_alloc = Filesystem::allocate();
    let vfs = FilesystemWith::mount(&mut volatile_fs_alloc, &mut volatile_storage)
        .expect("could not mount volatile storage");

    let mut service = Service::new(rng, ifs, efs, vfs).expect("service init worked");
    assert!(service.add_endpoint(service_endpoint).is_ok());

    let mut raw_client = client::RawClient::new(client_endpoint);
    f(&mut service, &mut raw_client);
}

fn setup<F>(f: F)
where F: Fn(
    // &mut Service::<'_, '_, tests::MockRng, tests::InternalStorage<'_>, tests::ExternalStorage<'_>, tests::VolatileStorage<'_>>,
    &mut Client<'_, &mut service::Service<'_, '_, tests::MockRng, tests::InternalStorage<'_>, tests::ExternalStorage<'_>, tests::VolatileStorage<'_>>>
) {

    // whole lotta setup goin' on ;)

    let mut request_queue = heapless::spsc::Queue(heapless::i::Queue::u8());
    let mut reply_queue = heapless::spsc::Queue(heapless::i::Queue::u8());
    let (service_endpoint, client_endpoint) = pipe::new_endpoints(&mut request_queue, &mut reply_queue, "fido2");
    let rng = MockRng::new();

    let mut internal_ram = InternalRam::default();
    let mut internal_storage = InternalStorage::new(&mut internal_ram);
    Filesystem::format(&mut internal_storage).expect("could not format internal storage");
    let mut internal_fs_alloc = Filesystem::allocate();
    let ifs = FilesystemWith::mount(&mut internal_fs_alloc, &mut internal_storage)
        .expect("could not mount internal storage");

    let mut external_ram = ExternalRam::default();
    let mut external_storage = ExternalStorage::new(&mut external_ram);
    Filesystem::format(&mut external_storage).expect("could not format external storage");
    let mut external_fs_alloc = Filesystem::allocate();
    let efs = FilesystemWith::mount(&mut external_fs_alloc, &mut external_storage)
        .expect("could not mount external storage");

    let mut volatile_ram = VolatileRam::default();
    let mut volatile_storage = VolatileStorage::new(&mut volatile_ram);
    Filesystem::format(&mut volatile_storage).expect("could not format volatile storage");
    let mut volatile_fs_alloc = Filesystem::allocate();
    let vfs = FilesystemWith::mount(&mut volatile_fs_alloc, &mut volatile_storage)
        .expect("could not mount volatile storage");

    let mut service = Service::new(rng, ifs, efs, vfs).expect("service init worked");
    assert!(service.add_endpoint(service_endpoint).is_ok());

    // let mut raw_client = client::RawClient::new(client_endpoint);
    // f(&mut service, &mut client);

    let syscaller = &mut service;
    let mut client = Client::new(client_endpoint, syscaller);
    f(&mut client);
}

#[test]
fn dummy() {
    raw_setup(|service, client| {
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
    });
}

// #[test]
// fn sign_ed25519_raw() {
//     raw_setup(|service, client| {
//     // let (service_endpoint, client_endpoint) = pipe::new_endpoints(
//     //     unsafe { &mut REQUEST_PIPE },
//     //     unsafe { &mut REPLY_PIPE },
//     //     "fido2",
//     // );

//     // let rng = MockRng::new();

//     // // need to figure out if/how to do this as `static mut`
//     // let mut persistent_ram = PersistentRam::default();
//     // let mut persistent_storage = PersistentStorage::new(&mut persistent_ram);
//     // Filesystem::format(&mut persistent_storage).expect("could not format persistent storage");
//     // let mut persistent_fs_alloc = Filesystem::allocate();
//     // let pfs = FilesystemWith::mount(&mut persistent_fs_alloc, &mut persistent_storage)
//     //     .expect("could not mount persistent storage");
//     // let mut volatile_ram = VolatileRam::default();
//     // let mut volatile_storage = VolatileStorage::new(&mut volatile_ram);
//     // Filesystem::format(&mut volatile_storage).expect("could not format volatile storage");
//     // let mut volatile_fs_alloc = Filesystem::allocate();
//     // let vfs = FilesystemWith::mount(&mut volatile_fs_alloc, &mut volatile_storage)
//     //     .expect("could not mount volatile storage");

//     // let mut service = Service::new(rng, ifs, efs, vfs);
//     // service.add_endpoint(service_endpoint).ok();
//     // let mut client = RawClient::new(client_endpoint);

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
//         message: Message::from_slice(&message).expect("all good"),
//     };

//     let mut future = request.submit(&mut client).map_err(drop).unwrap();
//     service.process();
//     let reply = block!(future);
//     });
// }

#[test]
fn sign_ed25519() {
    setup(|client| {
        let mut future = client.generate_ed25519_private_key(StorageLocation::Internal).expect("no client error");
        println!("submitted gen ed25519");
        let reply = block!(future);
        let private_key = reply.expect("no errors, never").key;
        println!("got a private key {:?}", &private_key);

        let public_key = block!(client.derive_ed25519_public_key(&private_key, StorageLocation::Volatile).expect("no client error"))
            .expect("no issues").key;
        println!("got a public key {:?}", &public_key);

        assert!(block!(
                client.derive_ed25519_public_key(&private_key, StorageLocation::Volatile).expect("no client error wot")
        ).is_ok());
        assert!(block!(
                client.derive_p256_public_key(&private_key, StorageLocation::Volatile).expect("no client error wot")
        ).is_err());

        let message = [1u8, 2u8, 3u8];
        let mut future = client.sign_ed25519(&private_key, &message).expect("no client error post err");
        let reply: Result<api::reply::Sign, _> = block!(future);
        let signature = reply.expect("good signature").signature;
        println!("got a signature: {:?}", &signature);

        let mut future = client.verify_ed25519(&public_key, &message, &signature).expect("no client error");
        let reply = block!(future);
        let valid = reply.expect("good signature").valid;
        assert!(valid);

        let mut future = client.verify_ed25519(&public_key, &message, &[1u8,2,3]).expect("no client error");
        let reply = block!(future);
        assert_eq!(Err(Error::WrongSignatureLength), reply);
    });
}

#[test]
fn sign_p256() {
    setup(|client| {
        let private_key = block!(client.generate_p256_private_key(StorageLocation::External).expect("no client error"))
            .expect("no errors").key;
        println!("got a public key {:?}", &private_key);
        let public_key = block!(client.derive_p256_public_key(&private_key, StorageLocation::Volatile).expect("no client error"))
            .expect("no errors").key;
        println!("got a public key {:?}", &public_key);

        let message = [1u8, 2u8, 3u8];
        let signature = block!(client.sign_p256(&private_key, &message).expect("no client error"))
            .expect("good signature")
            .signature;

        // use core::convert::AsMut;
        // let sig = signature.0.as_mut()[0] = 0;
        let future = client.verify_p256(&public_key, &message, &signature);
        let mut future = future.expect("no client error");
        let result = block!(future);
        if result.is_err() {
            println!("error: {:?}", result);
        }
        let reply = result.expect("valid signature");
        let valid = reply.valid;
        assert!(valid);
    });
}

#[test]
fn agree_p256() {
    setup(|client| {
        let plat_private_key = block!(client.generate_p256_private_key(StorageLocation::Volatile).expect("no client error"))
            .expect("no errors").key;
        println!("got a public key {:?}", &plat_private_key);
        let plat_public_key = block!(client.derive_p256_public_key(&plat_private_key, StorageLocation::Volatile).expect("no client error"))
            .expect("no errors").key;
        println!("got a public key {:?}", &plat_public_key);

        let auth_private_key = block!(client.generate_p256_private_key(StorageLocation::Volatile).expect("no client error"))
            .expect("no errors").key;
        println!("got a public key {:?}", &auth_private_key);
        let auth_public_key = block!(client.derive_p256_public_key(&auth_private_key, StorageLocation::Volatile).expect("no client error"))
            .expect("no errors").key;
        println!("got a public key {:?}", &auth_public_key);

        let shared_secret = block!(
            client.agree(Mechanism::P256, auth_private_key.clone(), plat_public_key.clone(),
                         StorageAttributes::new().set_persistence(StorageLocation::Volatile))
                .expect("no client error"))
            .expect("no errors").shared_secret;

        let alt_shared_secret = block!(
            client.agree(Mechanism::P256, plat_private_key.clone(), auth_public_key.clone(),
                         StorageAttributes::new().set_persistence(StorageLocation::Volatile))
                .expect("no client error"))
            .expect("no errors").shared_secret;

        // NB: we have no idea about the value of keys, these are just *different* handles
        assert_ne!(&shared_secret, &alt_shared_secret);

        let symmetric_key = block!(
            client.derive_key(Mechanism::Sha256, shared_secret.clone(),
                              StorageAttributes::new().set_persistence(StorageLocation::Volatile))
                .expect("no client error"))
            .expect("no errors").key;

        let new_pin_enc = [1u8, 2, 3];

        let _tag = block!(
            client.sign(Mechanism::HmacSha256, symmetric_key.clone(), &new_pin_enc)
                .expect("no client error"))
            .expect("no errors").signature;
    });
}

#[test]
fn aead() {
    setup(|client| {
        let secret_key =
            block!(
                client
                .generate_chacha8poly1305_key(StorageLocation::Volatile)
                .expect("no client error")
            )
            .expect("no errors")
            .key;

        println!("got a key {:?}", &secret_key);

        let message = b"test message";
        let associated_data = b"solokeys.com";
        let api::reply::Encrypt { ciphertext, nonce, tag } =
            block!(client.encrypt_chacha8poly1305(&secret_key, message, associated_data).expect("no client error"))
            .expect("no errors");

        let plaintext =
            block!(client.decrypt_chacha8poly1305(
                    &secret_key,
                    &ciphertext,
                    associated_data,
                    &nonce,
                    &tag,
                 ).map_err(drop).expect("no client error"))
            .map_err(drop).expect("no errors").plaintext;

        assert_eq!(&message[..], plaintext.as_ref());
    });
}
