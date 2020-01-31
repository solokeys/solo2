 #![cfg_attr(not(test), no_std)]

use cortex_m_semihosting::hprintln;

use littlefs2::{
    driver::Storage,
    fs::{
        File,
        FileWith,
        FilesystemWith,
    },
    io::{
        Error as FsError,
        Result as FsResult,
        ReadWith,
        WriteWith,
    },
};

use usbd_ctaphid::{
    authenticator::{
        self,
        Error,
        Result,
    },
    types::{
        AssertionResponses,
        AttestationObject,
        AuthenticatorInfo,
        GetAssertionParameters,
        MakeCredentialParameters,
    },
};

type MasterSecret = [u8; 32];
const MASTER_SECRET_FILENAME: &'static str = "/master-secret";

type SignatureCounter = u32;
const SIGNATURE_COUNTER_FILENAME: &'static str = "/signature-counter";

pub struct Authenticator<'fs, 'storage, R, S>
where
    R: embedded_hal::blocking::rng::Read,
    S: Storage,
{
    fs: FilesystemWith<'fs, 'storage, S>,
    rng: R,
    // aaguid: Bytes<consts::U16>,
}

impl<'fs, 'storage, R, S> Authenticator<'fs, 'storage, R, S>
where
    R: embedded_hal::blocking::rng::Read,
    S: Storage,
{
    pub fn init(
        fs: FilesystemWith<'fs, 'storage, S>,
        rng: R,
    ) -> Self {
        let mut authenticator = Authenticator {
            fs,
            rng,
        };
        authenticator.ensure_master_secret();
        hprintln!("sig counter = {}, incrementing for good measure", authenticator.ensure_signature_counter()).ok();
        hprintln!("new: {}", authenticator.increment_signature_counter().expect("could not increment")).ok();
        authenticator
    }

    fn ensure_master_secret(&mut self) {
        match self.fs.metadata(MASTER_SECRET_FILENAME) {
            Ok(_) => {
                hprintln!("master secret: starts with {:?}", &self.master_secret().expect("failed to read master secret")[..3]).ok();
            },
            Err(FsError::NoSuchEntry) => {
                hprintln!("no such entry {:?}", MASTER_SECRET_FILENAME).ok();
                match self.set_master_secret() {
                    Ok(_) => {},
                    Err(_) => {
                        panic!("oh noes");
                    }
                }
            },
            _ => {
                panic!("oh no!");
            }

        }
    }

    fn master_secret(&mut self) -> FsResult<MasterSecret> {
        let mut alloc = File::allocate();
        // may use common `OpenOptions`
        let mut file = FileWith::open(MASTER_SECRET_FILENAME, &mut alloc, &mut self.fs)?;
        let mut secret: MasterSecret = Default::default();
        file.read(&mut secret)?;
        Ok(secret)
    }

    fn set_master_secret(&mut self) -> FsResult<()>{
        let mut secret: MasterSecret = Default::default();
        if self.rng.read(&mut secret).is_err() {
            panic!("RNG malfunction");
        }
        hprintln!("generated master secret: {:?}", &secret[..]).ok();

        let mut alloc = File::allocate();
        let mut file = FileWith::create(MASTER_SECRET_FILENAME, &mut alloc, &mut self.fs)?;

        file.write(&mut secret)?;
        file.sync()
    }

    fn ensure_signature_counter(&mut self) -> SignatureCounter {
        match self.signature_counter() {
            Ok(counter) => { counter },
            Err(_) => {
                self.set_signature_counter(0).expect("could not ensure signature counter");
                0
            }
        }
    }

    fn signature_counter(&mut self) -> FsResult<SignatureCounter> {
        let mut alloc = File::allocate();
        // may use common `OpenOptions`
        let mut file = FileWith::open(SIGNATURE_COUNTER_FILENAME, &mut alloc, &mut self.fs)?;
        let mut counter_as_bytes: [u8; 4] = Default::default();
        file.read(&mut counter_as_bytes)?;
        Ok(u32::from_le_bytes(counter_as_bytes))
    }

    fn set_signature_counter(&mut self, counter: SignatureCounter) -> FsResult<()>{
        let mut alloc = File::allocate();
        let mut file = FileWith::create(SIGNATURE_COUNTER_FILENAME, &mut alloc, &mut self.fs)?;

        file.write(&mut counter.to_le_bytes())?;
        file.sync()
    }

    fn increment_signature_counter(&mut self) -> FsResult<SignatureCounter> {
        let mut counter = self.signature_counter()?;
        counter += 1;
        self.set_signature_counter(counter)?;
        Ok(counter)
    }

}

impl<'fs, 'storage, R, S> authenticator::Api for Authenticator<'fs, 'storage, R, S>
where
    R: embedded_hal::blocking::rng::Read,
    S: Storage,
{
    fn get_info(&self) -> AuthenticatorInfo {
        todo!();
    }

    fn reset(&mut self) -> Result<()> {
        todo!();
    }


    fn get_assertions(&mut self, params: &GetAssertionParameters) -> Result<AssertionResponses> {
        todo!();
    }

    fn make_credential(&mut self, params: &MakeCredentialParameters) -> Result<AttestationObject> {
        todo!();
    }
}

#[cfg(test)]
mod test {
}
