use core::convert::TryInto;
use core::convert::TryFrom;

use crate::api::*;
use crate::config::*;
use crate::error::Error;
use crate::mechanisms;
use crate::types::*;

pub use crate::pipe::ServiceEndpoint;

use chacha20poly1305::ChaCha8Poly1305;
pub use embedded_hal::blocking::rng::Read as RngRead;

// #[macro_use]
// mod macros;

pub trait Agree<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> {
    fn agree(_resources: &mut ServiceResources<'a, 's, R, P, V>, _request: request::Agree)
    -> Result<reply::Agree, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Decrypt<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> {
    fn decrypt(_resources: &mut ServiceResources<'a, 's, R, P, V>, _request: request::Decrypt)
    -> Result<reply::Decrypt, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait DeriveKey<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> {
    fn derive_key(_resources: &mut ServiceResources<'a, 's, R, P, V>, _request: request::DeriveKey)
    -> Result<reply::DeriveKey, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Encrypt<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> {
    fn encrypt(_resources: &mut ServiceResources<'a, 's, R, P, V>, _request: request::Encrypt)
    -> Result<reply::Encrypt, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait GenerateKey<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> {
    fn generate_key(_resources: &mut ServiceResources<'a, 's, R, P, V>, _request: request::GenerateKey)
    -> Result<reply::GenerateKey, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Sign<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> {
    fn sign(_resources: &mut ServiceResources<'a, 's, R, P, V>, _request: request::Sign)
    -> Result<reply::Sign, Error> { Err(Error::MechanismNotAvailable) }
}

pub trait Verify<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> {
    fn verify(_resources: &mut ServiceResources<'a, 's, R, P, V>, _request: request::Verify)
    -> Result<reply::Verify, Error> { Err(Error::MechanismNotAvailable) }
}

// associated keys end up namespaced under "/fido2"
// example: "/fido2/keys/2347234"
// let (mut fido_endpoint, mut fido2_client) = Client::new("fido2");
// let (mut piv_endpoint, mut piv_client) = Client::new("piv");

pub struct ServiceResources<'a, 's, Rng, PersistentStorage, VolatileStorage>
where
    Rng: RngRead,
    PersistentStorage: LfsStorage,
    VolatileStorage: LfsStorage,
{
    pub(crate) rng: Rng,
    // maybe make this more flexible later, but not right now
    // cryptoki: "token objects"
    #[allow(dead_code)]
    pfs: FilesystemWith<'s, 's, PersistentStorage>,
    // cryptoki: "session objects"
    vfs: FilesystemWith<'s, 's, VolatileStorage>,
    currently_serving: &'a str,
}

pub struct Service<'a, 's, Rng, PersistentStorage, VolatileStorage>
where
    Rng: RngRead,
    PersistentStorage: LfsStorage,
    VolatileStorage: LfsStorage,
{
    eps: Vec<ServiceEndpoint<'a>, MAX_SERVICE_CLIENTS>,
    resources: ServiceResources<'a, 's, Rng, PersistentStorage, VolatileStorage>,
}

impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> ServiceResources<'a, 's, R, P, V> {

    pub fn try_new(
        rng: R,
        pfs: FilesystemWith<'s, 's, P>,
        vfs: FilesystemWith<'s, 's, V>,
    ) -> Result<Self, Error> {

        Ok(Self { rng, pfs, vfs, currently_serving: &"" })
    }

    // TODO: key a `/root/aead-key`
    pub fn get_aead_key(&self) -> Result<AeadKey, Error> {
        Ok([37u8; 32])
    }

    // TODO: key a `/root/aead-nonce` counter (or use entropy?)
    pub fn get_aead_nonce(&self) -> Result<AeadNonce, Error> {
        Ok([42u8; 12])
    }

    // global choice of algorithm: we do Chacha8Poly1305 here
    // TODO: oh how annoying these GenericArrays
    pub fn aead_in_place(&mut self, ad: &[u8], buf: &mut [u8]) -> Result<(AeadNonce, AeadTag), Error> {
        use chacha20poly1305::aead::{Aead, NewAead};

        // keep in state?
        let aead = ChaCha8Poly1305::new(GenericArray::clone_from_slice(&self.get_aead_key()?));
        // auto-increments
        let nonce = self.get_aead_nonce()?;

        // aead.encrypt_in_place_detached(&nonce, ad, buf).map(|g| g.as_slice().try_into().unwrap())?;
        // not sure what can go wrong with AEAD
        let tag: AeadTag = aead.encrypt_in_place_detached(
            &GenericArray::clone_from_slice(&nonce), ad, buf
        ).unwrap().as_slice().try_into().unwrap();
        Ok((nonce, tag))
    }

    pub fn adad_in_place(&mut self, nonce: &AeadNonce, ad: &[u8], buf: &mut [u8], tag: &AeadTag) -> Result<(), Error> {
        use chacha20poly1305::aead::{Aead, NewAead};

        // keep in state?
        let aead = ChaCha8Poly1305::new(GenericArray::clone_from_slice(&self.get_aead_key()?));

        aead.decrypt_in_place_detached(
            &GenericArray::clone_from_slice(nonce),
            ad,
            buf,
            &GenericArray::clone_from_slice(tag)
        ).map_err(|_| Error::AeadError)
    }

    pub fn reply_to(&mut self, request: Request) -> Result<Reply, Error> {
        // TODO: what we want to do here is map an enum to a generic type
        // Is there a nicer way to do this?
        match request {
            Request::DummyRequest => {
                #[cfg(test)]
                println!("got a dummy request!");
                Ok(Reply::DummyReply)
            },

            Request::Agree(request) => {
                match request.mechanism {

                    Mechanism::P256 => mechanisms::P256::agree(self, request),
                    _ => return Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Agree(reply))
            },

            Request::Decrypt(request) => {
                match request.mechanism {

                    Mechanism::Aes256Cbc => mechanisms::Aes256Cbc::decrypt(self, request),
                    _ => return Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Decrypt(reply))
            },

            Request::DeriveKey(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::derive_key(self, request),
                    Mechanism::P256 => mechanisms::P256::derive_key(self, request),
                    Mechanism::Sha256 => mechanisms::Sha256::derive_key(self, request),
                    _ => return Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::DeriveKey(reply))
            },

            Request::Encrypt(request) => {
                match request.mechanism {

                    Mechanism::Aes256Cbc => mechanisms::Aes256Cbc::encrypt(self, request),
                    _ => return Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Encrypt(reply))
            },

            Request::GenerateKey(request) => {
                match request.mechanism {
                    Mechanism::Ed25519 => mechanisms::Ed25519::generate_key(self, request),
                    Mechanism::P256 => mechanisms::P256::generate_key(self, request),
                    _ => Err(Error::MechanismNotAvailable),
                }.map(|reply| Reply::GenerateKey(reply))
            },

            Request::Sign(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::sign(self, request),
                    Mechanism::HmacSha256 => mechanisms::HmacSha256::sign(self, request),
                    Mechanism::P256 => mechanisms::P256::sign(self, request),
                    _ => return Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Sign(reply))
            },

            Request::Verify(request) => {
                match request.mechanism {

                    Mechanism::Ed25519 => mechanisms::Ed25519::verify(self, request),
                    Mechanism::P256 => mechanisms::P256::verify(self, request),
                    _ => return Err(Error::MechanismNotAvailable),

                }.map(|reply| Reply::Verify(reply))
            },

            _ => {
                #[cfg(test)]
                println!("todo: {:?} request!", &request);
                Err(Error::RequestNotAvailable)
            },
        }
    }

    pub fn prepare_path_for_key(&mut self, key_type: KeyType, id: &UniqueId)
        -> Result<Bytes<MAX_PATH_LENGTH>, Error> {
        let mut path = Bytes::<MAX_PATH_LENGTH>::new();
        path.extend_from_slice(b"/").map_err(|_| Error::InternalError)?;
        path.extend_from_slice(self.currently_serving.as_bytes()).map_err(|_| Error::InternalError)?;
        // #[cfg(all(test, feature = "verbose-tests"))]
        // #[cfg(test)]
        // println!("creating dir {:?}", &path);
        // self.pfs.create_dir(path.as_ref()).map_err(|_| Error::FilesystemWriteFailure)?;

        path.extend_from_slice(match key_type {
            KeyType::Private => b"-private",
            KeyType::Public => b"-public",
            KeyType::Secret => b"-secret",
        }).map_err(|_| Error::InternalError)?;

        // #[cfg(all(test, feature = "verbose-tests"))]
        // println!("creating dir {:?}", &path);
        // self.pfs.create_dir(path.as_ref()).map_err(|_| Error::FilesystemWriteFailure)?;
        path.extend_from_slice(b"-").map_err(|_| Error::InternalError)?;
        path.extend_from_slice(&id.hex()).map_err(|_| Error::InternalError)?;
        Ok(path)
    }

    pub fn generate_unique_id(&mut self) -> Result<UniqueId, Error> {
        let mut unique_id = [0u8; 16];

        self.rng.read(&mut unique_id)
            .map_err(|_| Error::EntropyMalfunction)?;

        #[cfg(all(test, feature = "verbose-tests"))]
        println!("unique id {:?}", &unique_id);
        Ok(UniqueId(unique_id))
    }

    pub fn load_serialized_key(&mut self, path: &[u8], serialized_key: &mut [u8]) -> Result<(), Error> {
        #[cfg(test)]
        // actually safe, as path is ASCII by construction
        println!("loading from file {:?}", unsafe { core::str::from_utf8_unchecked(&path[..]) });

        use littlefs2::fs::{File, FileWith};
        let mut alloc = File::allocate();
        let mut file = FileWith::open(&path[..], &mut alloc, &mut self.vfs)
            .map_err(|_| Error::FilesystemReadFailure)?;

        use littlefs2::io::ReadWith;
        file.read(serialized_key)
            .map_err(|_| Error::FilesystemReadFailure)?;

        Ok(())
    }

    // TODO: in the case of desktop/ram storage:
    // - using file.sync (without file.close) leads to an endless loop
    // - this loop happens inside `lfs_dir_commit`, namely inside its first for loop
    //   https://github.com/ARMmbed/littlefs/blob/v2.1.4/lfs.c#L1680-L1694
    // - the `if` condition is never fulfilled, it seems f->next continues "forever"
    //   through whatever lfs->mlist is.
    //
    // see also https://github.com/ARMmbed/littlefs/issues/145
    //
    // OUTCOME: either ensure calling `.close()`, or patch the call in a `drop` for FileWith.
    //
    pub fn store_serialized_key(&mut self, path: &[u8], serialized_key: &[u8]) -> Result<(), Error> {
        // actually safe, as path is ASCII by construction
        #[cfg(test)]
        println!("storing in file {:?}", unsafe { core::str::from_utf8_unchecked(&path[..]) });

        use littlefs2::fs::{File, FileWith};
        let mut alloc = File::allocate();
        // #[cfg(test)]
        // println!("allocated FileAllocation");
        let mut file = FileWith::create(&path[..], &mut alloc, &mut self.vfs)
            .map_err(|_| Error::FilesystemWriteFailure)?;
        // #[cfg(test)]
        // println!("created file");
        use littlefs2::io::WriteWith;
        file.write(&serialized_key)
            .map_err(|_| Error::FilesystemWriteFailure)?;
        // #[cfg(test)]
        // println!("wrote file");
        file.sync()
            .map_err(|_| Error::FilesystemWriteFailure)?;
        // #[cfg(test)]
        // println!("sync'd file");
        // file.close()
        //     .map_err(|_| Error::FilesystemWriteFailure)?;
        // #[cfg(test)]
        // println!("closed file");

        Ok(())
    }
}

impl<'a, 's, R: RngRead, P: LfsStorage, V: LfsStorage> Service<'a, 's, R, P, V> {

    pub fn new(
        rng: R,
        persistent_storage: FilesystemWith<'s, 's, P>,
        volatile_storage: FilesystemWith<'s, 's, V>,
    )
        -> Result<Self, Error>
    {
        let resources = ServiceResources::try_new(rng, persistent_storage, volatile_storage)?;
        Ok(Self { eps: Vec::new(), resources, })
    }

    pub fn add_endpoint(&mut self, ep: ServiceEndpoint<'a>) -> Result<(), ServiceEndpoint> {
        self.eps.push(ep)
    }

    // process one request per client which has any
    pub fn process(&mut self) {
        // split self since we iter-mut over eps and need &mut of the other resources
        let eps = &mut self.eps;
        let mut resources = &mut self.resources;

        for ep in eps.iter_mut() {
            if !ep.send.ready() {
                continue;
            }
            if let Some(request) = ep.recv.dequeue() {
                #[cfg(test)]
                println!("service got request: {:?}", &request);
                resources.currently_serving = &ep.client_id;
                let reply_result = resources.reply_to(request);
                ep.send.enqueue(reply_result).ok();
            }
        }
    }
}

impl<'a, 's, R, P, V> crate::pipe::Syscall for &mut Service<'a, 's, R, P, V>
where
    R: RngRead,
    P: LfsStorage,
    V: LfsStorage,
{
    fn syscall(&mut self) {
        self.process();
    }
}
