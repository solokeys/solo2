#![cfg(test)]

use std::convert::TryInto;

use chacha20::ChaCha20;

use crate::*;
use crate::types::*;
use littlefs2::fs::{Allocation, Filesystem};
use littlefs2::const_ram_storage;
use interchange::Interchange;



pub struct MockRng(ChaCha20);

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



#[derive(Default)]
pub struct UserInterface {
}

impl crate::traits::platform::UserInterface for UserInterface
{
    fn check_user_presence(&mut self) -> consent::Level {
        consent::Level::Normal
    }

    fn set_status(&mut self, status: ui::Status) {

        println!("Set status: {:?}", status);

    }

    fn refresh(&mut self) {

    }

    fn uptime(&mut self) -> core::time::Duration {
        core::time::Duration::from_millis(1000)
    }

    fn reboot(&mut self, to: reboot::To) -> ! {
        println!("Restart!  ({:?})", to);
        std::process::exit(25);
    }

}



// Using macro to avoid maintaining the type declarations
macro_rules! setup {
    ($client:ident) => {
            const_ram_storage!(InternalStorage, 4096*10);
            const_ram_storage!(ExternalStorage, 4096*10);
            const_ram_storage!(VolatileStorage, 4096*10);

            store!(Store,
                Internal: InternalStorage,
                External: ExternalStorage,
                Volatile: VolatileStorage
            );
            board!(Board,
                R: MockRng,
                S: Store,
                UI: UserInterface,
            );
            pub type TestClient<'a> = crate::DefaultClient<&'a mut crate::Service<Board>>;

            let filesystem = InternalStorage::new();

            static mut INTERNAL_STORAGE: Option<InternalStorage> = None;
            unsafe { INTERNAL_STORAGE = Some(filesystem); }
            static mut INTERNAL_FS_ALLOC: Option<Allocation<InternalStorage>> = None;
            unsafe { INTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }

            static mut EXTERNAL_STORAGE: ExternalStorage = ExternalStorage::new();
            static mut EXTERNAL_FS_ALLOC: Option<Allocation<ExternalStorage>> = None;
            unsafe { EXTERNAL_FS_ALLOC = Some(Filesystem::allocate()); }

            static mut VOLATILE_STORAGE: VolatileStorage = VolatileStorage::new();
            static mut VOLATILE_FS_ALLOC: Option<Allocation<VolatileStorage>> = None;
            unsafe { VOLATILE_FS_ALLOC = Some(Filesystem::allocate()); }


            let store = Store::claim().unwrap();

            store.mount(
                unsafe { INTERNAL_FS_ALLOC.as_mut().unwrap() },
                // unsafe { &mut INTERNAL_STORAGE },
                unsafe { INTERNAL_STORAGE.as_mut().unwrap() },
                unsafe { EXTERNAL_FS_ALLOC.as_mut().unwrap() },
                unsafe { &mut EXTERNAL_STORAGE },
                unsafe { VOLATILE_FS_ALLOC.as_mut().unwrap() },
                unsafe { &mut VOLATILE_STORAGE },
                // to trash existing data, set to true
                true,
            ).unwrap();

            let rng = MockRng::new();
            let pc_interface: UserInterface = Default::default();

            let board = Board::new(rng, store, pc_interface);
            let mut trussed: crate::Service<Board> = crate::service::Service::new(board);

            let (test_trussed_requester, test_trussed_responder) = crate::pipe::TrussedInterchange::claim(0)
                .expect("could not setup TEST TrussedInterchange");
            let mut test_client_id = littlefs2::path::PathBuf::new();
            test_client_id.push(b"TEST\0".try_into().unwrap());

            assert!(trussed.add_endpoint(test_trussed_responder, test_client_id).is_ok());

            let mut $client = TestClient::new(
                test_trussed_requester,
                &mut trussed
            );

    }
}

#[test]
#[serial]
fn dummy() {

    setup!(_client);

 }

#[test]
#[serial]
fn sign_ed25519() {
    // let mut client = setup!();
    setup!(client);

    let future = client.generate_ed25519_private_key(StorageLocation::Internal).expect("no client error");
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
    let future = client.sign_ed25519(&private_key, &message).expect("no client error post err");
    let reply: Result<api::reply::Sign, _> = block!(future);
    let signature = reply.expect("good signature").signature;
    println!("got a signature: {:?}", &signature);

    let future = client.verify_ed25519(&public_key, &message, &signature).expect("no client error");
    let reply = block!(future);
    let valid = reply.expect("good signature").valid;
    assert!(valid);

    let future = client.verify_ed25519(&public_key, &message, &[1u8,2,3]).expect("no client error");
    let reply = block!(future);
    assert_eq!(Err(Error::WrongSignatureLength), reply);
}

#[test]
#[serial]
fn sign_p256() {
    // let mut client = setup!();
    setup!(client);
        let private_key = block!(client.generate_p256_private_key(StorageLocation::External).expect("no client error"))
            .expect("no errors").key;
        println!("got a public key {:?}", &private_key);
        let public_key = block!(client.derive_p256_public_key(&private_key, StorageLocation::Volatile).expect("no client error"))
            .expect("no errors").key;
        println!("got a public key {:?}", &public_key);

        let message = [1u8, 2u8, 3u8];
        let signature = block!(client.sign_p256(&private_key, &message, SignatureSerialization::Raw)
            .expect("no client error"))
            .expect("good signature")
            .signature;

        // use core::convert::AsMut;
        // let sig = signature.0.as_mut()[0] = 0;
        let future = client.verify_p256(&public_key, &message, &signature);
        let future = future.expect("no client error");
        let result = block!(future);
        if result.is_err() {
            println!("error: {:?}", result);
        }
        let reply = result.expect("valid signature");
        let valid = reply.valid;
        assert!(valid);
}

#[test]
#[serial]
fn agree_p256() {
    // let mut client = setup!();
    setup!(client);
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
            client.sign(Mechanism::HmacSha256, symmetric_key.clone(), &new_pin_enc, SignatureSerialization::Raw)
                .expect("no client error"))
            .expect("no errors").signature;
}

#[test]
#[serial]
fn aead() {
    // let mut client = setup!();
    setup!(client);
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
            block!(client.encrypt_chacha8poly1305(&secret_key, message, associated_data, None).expect("no client error"))
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

        assert_eq!(&message[..], plaintext.unwrap().as_slice());
}
