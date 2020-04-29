use core::convert::TryInto;

use cortex_m_semihosting::hprintln;

use crypto_service::{
    // Client as CryptoClient,
    pipe::Syscall as CryptoSyscall,
    types::{
        StorageLocation,
    },
};

use ctap_types::{
    authenticator::{
        Error,
        ctap2::{
            self,
            credential_management::*,
        },
    },
    webauthn::{
        PublicKeyCredentialDescriptor,
    },
};

use littlefs2::path::PathBuf;

use crate::{
    Authenticator,
    Result,
    UserPresence,
    credential::Credential,
    state::CommandCache,
};

#[macro_use]
macro_rules! syscall {
    ($pre_future_result:expr) => {{
        // evaluate the expression
        let mut future_result = $pre_future_result.expect("no client error");
        loop {
            match future_result.poll() {
                // core::task::Poll::Ready(result) => { break result.expect("no errors"); },
                core::task::Poll::Ready(result) => { break result.unwrap(); },
                core::task::Poll::Pending => {},
            }
        }
    }}
}

pub fn get_creds_metadata<S, UP>(
    authnr: &mut Authenticator<'_, S, UP>,
) -> Result<Response>

where
    S: CryptoSyscall,
    UP: UserPresence
{

    let mut response: ctap2::credential_management::Response = Default::default();

    response.max_possible_remaining_residential_credentials_count =
        Some(authnr.state.persistent.max_resident_credentials_guesstimate());

    // count number of existing RKs
    todo!();

    Ok(response)
}

pub fn enumerate_rps_begin<S, UP>(
    authnr: &mut Authenticator<'_, S, UP>,
) -> Result<Response>

where
    S: CryptoSyscall,
    UP: UserPresence
{

    let mut response: ctap2::credential_management::Response = Default::default();
    // rp (0x03): PublicKeyCredentialRpEntity
    // rpIDHash (0x04) : RP ID SHA-256 hash.
    // totalRPs (0x05) : Total number of RPs present on the authenticator.

    let dir = PathBuf::from(b"rk");

    let maybe_first_rp = syscall!(authnr.crypto.read_dir_first(
        StorageLocation::Internal,
        dir,
        None,
    )).entry;

    response.total_rps = Some(match maybe_first_rp {
        None => {
            0
        }
        _ => {
            let mut num_rps = 1;
            loop {
                let maybe_next_rp = syscall!(authnr.crypto.read_dir_next())
                    .entry;
                match maybe_next_rp {
                    None => break,
                    _ => num_rps += 1,
                }
            }
            num_rps
        }
    });

    if let Some(rp) = maybe_first_rp {

        // load credential and extract rp and rpIdHash
        let maybe_first_credential = syscall!(authnr.crypto.read_dir_first(
            StorageLocation::Internal,
            PathBuf::from(rp.path()),
            None
        )).entry;

        match maybe_first_credential {
            None => panic!("chaos! disorder!"),
            Some(rk_entry) => {
                let serialized = syscall!(authnr.crypto.read_file(
                    StorageLocation::Internal,
                    rk_entry.path().into(),
                )).data;

                let credential = Credential::deserialize(&serialized)
                    // this may be a confusing error message
                    .map_err(|_| Error::InvalidCredential)?;

                let rp = credential.data.rp;

                response.rp_id_hash = Some(authnr.hash(&rp.id.as_ref())?);
                response.rp = Some(rp);

            }
        }

        // cache state for next call
        if let Some(total_rps) = response.total_rps {
            if total_rps > 1 {
                let rp_id_hash = response.rp_id_hash.as_ref().unwrap().clone();
                authnr.state.runtime.cache = Some(CommandCache::
                    CredentialManagementEnumerateRps(total_rps - 1, rp_id_hash));
            }
        }
    }

    Ok(response)
}

pub fn enumerate_rps_get_next_rp<S, UP>(
    authnr: &mut Authenticator<'_, S, UP>,
) -> Result<Response>

where
    S: CryptoSyscall,
    UP: UserPresence
{
    let (remaining, last_rp_id_hash) = match authnr.state.runtime.cache {
        Some(CommandCache::CredentialManagementEnumerateRps(
                remaining, ref rp_id_hash)) =>
            (remaining, rp_id_hash),
        _ => return Err(Error::InvalidCommand),
    };

    let dir = PathBuf::from(b"rk");

    let mut hex = [b'0'; 16];
    super::format_hex(&last_rp_id_hash[..8], &mut hex);
    let filename = PathBuf::from(&hex);

    let maybe_next_rp = syscall!(authnr.crypto.read_dir_first(
        StorageLocation::Internal,
        dir,
        Some(filename),
    )).entry;

    let mut response: ctap2::credential_management::Response = Default::default();

    if let Some(rp) = maybe_next_rp {
        // load credential and extract rp and rpIdHash
        let maybe_first_credential = syscall!(authnr.crypto.read_dir_first(
            StorageLocation::Internal,
            PathBuf::from(rp.path()),
            None
        )).entry;

        match maybe_first_credential {
            None => panic!("chaos! disorder!"),
            Some(rk_entry) => {
                let serialized = syscall!(authnr.crypto.read_file(
                    StorageLocation::Internal,
                    rk_entry.path().into(),
                )).data;

                let credential = Credential::deserialize(&serialized)
                    // this may be a confusing error message
                    .map_err(|_| Error::InvalidCredential)?;

                let rp = credential.data.rp;

                response.rp_id_hash = Some(authnr.hash(&rp.id.as_ref())?);
                response.rp = Some(rp);

                // cache state for next call
                if remaining > 1 {
                    let rp_id_hash = response.rp_id_hash.as_ref().unwrap().clone();
                    authnr.state.runtime.cache = Some(CommandCache::
                        CredentialManagementEnumerateRps(
                            remaining - 1, rp_id_hash));
                } else {
                    authnr.state.runtime.cache = None;

                }
            }
        }
    } else {
        authnr.state.runtime.cache = None;
    }

    Ok(response)
}

pub fn delete_credential<S, UP>(
    authnr: &mut Authenticator<'_, S, UP>,
    credential_descriptor: &PublicKeyCredentialDescriptor,
) -> Result<Response>

where
    S: CryptoSyscall,
    UP: UserPresence
{
    let credential_id_hash = authnr.hash(&credential_descriptor.id[..])?;
    let mut hex = [b'0'; 16];
    super::format_hex(&credential_id_hash[..8], &mut hex);
    let dir = PathBuf::from(b"rk");
    let filename = PathBuf::from(&hex);

    let rk_path = syscall!(authnr.crypto.locate_file(
        StorageLocation::Internal,
        Some(dir.clone()),
        filename,
    )).path.ok_or(Error::InvalidCredential)?;


    // DELETE
    authnr.delete_resident_key_by_path(&rk_path)?;

    // get rid of directory if it's now empty
    let rp_path = rk_path.parent()
        // by construction, RK has a parent, its RP
        .unwrap();

    let maybe_first_remaining_rk = syscall!(authnr.crypto.read_dir_first(
        StorageLocation::Internal,
        rp_path.clone(),
        None,
    )).entry;

    if maybe_first_remaining_rk.is_none() {
        // hprintln!("deleting parent {:?} as this was its last RK",
        //           &rp_path).ok();
        syscall!(authnr.crypto.remove_dir(
            StorageLocation::Internal,
            rp_path,
        ));
    }
    // just return OK
    let response: ctap2::credential_management::Response = Default::default();
    Ok(response)
}

