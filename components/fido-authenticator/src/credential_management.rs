//! TODO: There is potential need for `fsck`

use core::convert::TryFrom;

use cortex_m_semihosting::hprintln;

use trussed::{
    types::{
        DirEntry,
        StorageLocation,
    },
};

use ctap_types::{
    Bytes32,
    authenticator::{
        Error,
        ctap2::{
            self,
            credential_management::*,
        },
    },
    cose::PublicKey,
    webauthn::{
        PublicKeyCredentialDescriptor,
    },
};

use littlefs2::path::{Path, PathBuf};

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

pub struct CredentialManagement<'a, UP>
where
    UP: UserPresence,
{
    authnr: &'a mut Authenticator<UP>,
}

impl<'a, UP> core::ops::Deref for CredentialManagement<'a, UP>
where
    UP: UserPresence
{
    type Target = Authenticator<UP>;
    fn deref(&self) -> &Self::Target {
        &self.authnr
    }
}

impl<'a, UP> core::ops::DerefMut for CredentialManagement<'a, UP>
where
    UP: UserPresence
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.authnr
    }
}

impl<'a, UP> CredentialManagement<'a, UP>
where
    UP: UserPresence
{
    pub fn new(authnr: &'a mut Authenticator<UP>) -> Self {
        Self { authnr }
    }
}

impl<UP> CredentialManagement<'_, UP>
where
    UP: UserPresence
{
    pub fn get_creds_metadata(&mut self) -> Result<Response> {
        hprintln!("get metadata").ok();
        let mut response: ctap2::credential_management::Response =
            Default::default();

        let guesstimate = self.state.persistent
            .max_resident_credentials_guesstimate();
        response.existing_resident_credentials_count = Some(0);
        response.max_possible_remaining_residential_credentials_count =
            Some(guesstimate);

        let dir = PathBuf::from(b"rk");
        let maybe_first_rp = syscall!(self.crypto.read_dir_first(
            StorageLocation::Internal, dir.clone(), None)).entry;

        let first_rp = match maybe_first_rp{
            None => return Ok(response),
            Some(rp) => rp,
        };

        let (mut num_rks, _) = self.count_rp_rks(PathBuf::from(first_rp.path()))?;
        let mut last_rp = PathBuf::from(first_rp.file_name());

        loop {
            let maybe_next_rp = syscall!(self.crypto.read_dir_first(
                StorageLocation::Internal,
                dir.clone(),
                Some(last_rp),
            )).entry;

            match maybe_next_rp {
                None => {
                    response.existing_resident_credentials_count =
                        Some(num_rks);
                    response.max_possible_remaining_residential_credentials_count =
                        Some(if num_rks >= guesstimate {
                            0
                        } else {
                            guesstimate - num_rks
                        });
                    return Ok(response);
                }
                Some(rp) => {
                    last_rp = PathBuf::from(rp.file_name());
                    let (this_rp_rk_count, _) =
                        self.count_rp_rks(PathBuf::from(rp.path()))?;
                    num_rks += this_rp_rk_count;
                }
            }
        }
    }

    pub fn first_relying_party(&mut self) -> Result<Response> {
        hprintln!("first rp").ok();

        // rp (0x03): PublicKeyCredentialRpEntity
        // rpIDHash (0x04) : RP ID SHA-256 hash.
        // totalRPs (0x05) : Total number of RPs present on the authenticator.

        let mut response: ctap2::credential_management::Response =
            Default::default();

        let dir = PathBuf::from(b"rk");

        let maybe_first_rp = syscall!(self.crypto.read_dir_first(
            StorageLocation::Internal, dir, None)).entry;

        response.total_rps = Some(match maybe_first_rp {
            None => 0,
            _ => {
                let mut num_rps = 1;
                loop {
                    let maybe_next_rp = syscall!(self.crypto.read_dir_next())
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
            let maybe_first_credential = syscall!(self.crypto.read_dir_first(
                StorageLocation::Internal,
                PathBuf::from(rp.path()),
                None
            )).entry;

            match maybe_first_credential {
                None => panic!("chaos! disorder!"),
                Some(rk_entry) => {
                    let serialized = syscall!(self.crypto.read_file(
                        StorageLocation::Internal,
                        rk_entry.path().into(),
                    )).data;

                    let credential = Credential::deserialize(&serialized)
                        // this may be a confusing error message
                        .map_err(|_| Error::InvalidCredential)?;

                    let rp = credential.data.rp;

                    response.rp_id_hash = Some(self.hash(&rp.id.as_ref())?);
                    response.rp = Some(rp);

                }
            }

            // cache state for next call
            if let Some(total_rps) = response.total_rps {
                if total_rps > 1 {
                    let rp_id_hash = response.rp_id_hash.as_ref().unwrap().clone();
                    self.state.runtime.cache = Some(CommandCache::
                        CredentialManagementEnumerateRps(total_rps - 1, rp_id_hash));
                }
            }
        }

        Ok(response)
    }

    pub fn next_relying_party(&mut self) -> Result<Response> {
        hprintln!("next rp").ok();

        let (remaining, last_rp_id_hash) = match self.state.runtime.cache {
            Some(CommandCache::CredentialManagementEnumerateRps(
                    remaining, ref rp_id_hash)) =>
                (remaining, rp_id_hash),
            _ => return Err(Error::InvalidCommand),
        };

        let dir = PathBuf::from(b"rk");

        let mut hex = [b'0'; 16];
        super::format_hex(&last_rp_id_hash[..8], &mut hex);
        let filename = PathBuf::from(&hex);

        let maybe_next_rp = syscall!(self.crypto.read_dir_first(
            StorageLocation::Internal,
            dir,
            Some(filename),
        )).entry;

        let mut response: ctap2::credential_management::Response = Default::default();

        if let Some(rp) = maybe_next_rp {
            // load credential and extract rp and rpIdHash
            let maybe_first_credential = syscall!(self.crypto.read_dir_first(
                StorageLocation::Internal,
                PathBuf::from(rp.path()),
                None
            )).entry;

            match maybe_first_credential {
                None => panic!("chaos! disorder!"),
                Some(rk_entry) => {
                    let serialized = syscall!(self.crypto.read_file(
                        StorageLocation::Internal,
                        rk_entry.path().into(),
                    )).data;

                    let credential = Credential::deserialize(&serialized)
                        // this may be a confusing error message
                        .map_err(|_| Error::InvalidCredential)?;

                    let rp = credential.data.rp;

                    response.rp_id_hash = Some(self.hash(&rp.id.as_ref())?);
                    response.rp = Some(rp);

                    // cache state for next call
                    if remaining > 1 {
                        let rp_id_hash = response.rp_id_hash.as_ref().unwrap().clone();
                        self.state.runtime.cache = Some(CommandCache::
                            CredentialManagementEnumerateRps(
                                remaining - 1, rp_id_hash));
                    } else {
                        self.state.runtime.cache = None;
                    }
                }
            }
        } else {
            self.state.runtime.cache = None;
        }

        Ok(response)
    }

    fn count_rp_rks(&mut self, rp_dir: PathBuf) -> Result<(u32, DirEntry)> {
        let maybe_first_rk = syscall!(self.crypto.read_dir_first(
            StorageLocation::Internal,
            rp_dir,
            None
        )).entry;

        let first_rk = maybe_first_rk.ok_or(Error::NoCredentials)?;

        // count the rest of them
        let mut num_rks = 1;
        while syscall!(self.crypto.read_dir_next()).entry.is_some() {
            num_rks += 1;
        }
        Ok((num_rks, first_rk))
    }

    pub fn first_credential(&mut self, rp_id_hash: &Bytes32) -> Result<Response> {
        hprintln!("first credential").ok();

        self.state.runtime.cache = None;

        let mut hex = [b'0'; 16];
        super::format_hex(&rp_id_hash[..8], &mut hex);

        let rp_dir = PathBuf::from(b"rk").join(&PathBuf::from(&hex));
        let (num_rks, first_rk) = self.count_rp_rks(rp_dir)?;

        // extract data required into response
        let mut response = self.extract_response_from_credential_file(
            first_rk.path())?;
        response.total_credentials = Some(num_rks);

        // cache state for next call
        if let Some(num_rks) = response.total_credentials {
            if num_rks > 1 {
                // let rp_id_hash = response.rp_id_hash.as_ref().unwrap().clone();
                self.state.runtime.cache = Some(CommandCache::
                    CredentialManagementEnumerateCredentials(
                        num_rks - 1,
                        first_rk.path().parent().unwrap(),
                        PathBuf::from(first_rk.file_name()),
                    ));
            } else {
                self.state.runtime.cache = None;
            }
        }

        Ok(response)
    }

    pub fn next_credential(&mut self) -> Result<Response> {
        hprintln!("next credential").ok();

        let (remaining, rp_dir, prev_filename) = match self.state.runtime.cache {
            Some(CommandCache::CredentialManagementEnumerateCredentials(
                    x, ref y, ref z))
                 => (x, y.clone(), z.clone()),
            _ => return Err(Error::InvalidCommand),
        };

        self.state.runtime.cache = None;

        // let mut hex = [b'0'; 16];
        // super::format_hex(&rp_id_hash[..8], &mut hex);
        // let rp_dir = PathBuf::from(b"rk").join(&PathBuf::from(&hex));

        let maybe_next_rk = syscall!(self.crypto.read_dir_first(
            StorageLocation::Internal,
            rp_dir,
            Some(prev_filename)
        )).entry;

        match maybe_next_rk {
            Some(rk) => {
                // extract data required into response
                let response = self.extract_response_from_credential_file(
                    rk.path())?;

                // cache state for next call
                if remaining > 1 {
                    self.state.runtime.cache = Some(CommandCache::
                        CredentialManagementEnumerateCredentials(
                            remaining - 1,
                            rk.path().parent().unwrap(),
                            PathBuf::from(rk.file_name()),
                        )
                    );
                }

                Ok(response)
            }
            None => Err(Error::NoCredentials),
        }
    }


    fn extract_response_from_credential_file(&mut self, rk_path: &Path) -> Result<Response> {

        // user (0x06)
        // credentialID (0x07): PublicKeyCredentialDescriptor
        // publicKey (0x08): public key of the credential in COSE_Key format
        // totalCredentials (0x09): total number of credentials for this RP
        // credProtect (0x0A): credential protection policy

        let serialized = syscall!(self.crypto.read_file(
            StorageLocation::Internal,
            rk_path.into(),
        )).data;

        let credential = Credential::deserialize(&serialized)
            // this may be a confusing error message
            .map_err(|_| Error::InvalidCredential)?;

        // now fill response
        let mut response: ctap2::credential_management::Response =
            Default::default();

        response.user = Some(credential.data.user.clone());

        // why these contortions to get kek. sheesh
        let authnr = &mut self.authnr;
        let kek = authnr.state.persistent.key_encryption_key(&mut authnr.crypto)?;

        let credential_id =  credential.id(&mut self.crypto, &kek)?;
        response.credential_id = Some(credential_id.into());

        use crate::credential::Key;
        let private_key = match credential.key {
            Key::ResidentKey(key) => key,
            _ => return Err(Error::InvalidCredential),
        };

        use crate::SupportedAlgorithm;
        use trussed::types::{KeySerialization, Mechanism};

        let algorithm = SupportedAlgorithm::try_from(credential.algorithm)?;
        let cose_public_key =  match algorithm {
            SupportedAlgorithm::P256 => {
                let public_key = syscall!(self.crypto.derive_p256_public_key(&private_key, StorageLocation::Volatile)).key;
                let cose_public_key = syscall!(self.crypto.serialize_key(
                    Mechanism::P256, public_key.clone(),
                    // KeySerialization::EcdhEsHkdf256
                    KeySerialization::Cose,
                )).serialized_key;
                syscall!(self.crypto.delete(public_key));
                PublicKey::P256Key(
                    ctap_types::serde::cbor_deserialize(&cose_public_key)
                    .unwrap())
            }
            SupportedAlgorithm::Ed25519 => {
                let public_key = syscall!(self.crypto.derive_ed25519_public_key(&private_key, StorageLocation::Volatile)).key;
                let cose_public_key = syscall!(self.crypto.serialize_key(
                    Mechanism::Ed25519, public_key.clone(), KeySerialization::Cose
                )).serialized_key;
                syscall!(self.crypto.delete(public_key));
                PublicKey::Ed25519Key(
                    ctap_types::serde::cbor_deserialize(&cose_public_key)
                    .unwrap())
            }
        };
        response.public_key = Some(cose_public_key);
        // response.cred_protect = Some(credential.data.cred_protect as u8);

        Ok(response)
    }

    pub fn delete_credential(&mut self,
        credential_descriptor: &PublicKeyCredentialDescriptor,
    )
        -> Result<Response>
    {
        hprintln!("delete credential").ok();
        let credential_id_hash = self.hash(&credential_descriptor.id[..])?;
        let mut hex = [b'0'; 16];
        super::format_hex(&credential_id_hash[..8], &mut hex);
        let dir = PathBuf::from(b"rk");
        let filename = PathBuf::from(&hex);

        let rk_path = syscall!(self.crypto.locate_file(
            StorageLocation::Internal,
            Some(dir.clone()),
            filename,
        )).path.ok_or(Error::InvalidCredential)?;


        // DELETE
        self.delete_resident_key_by_path(&rk_path)?;

        // get rid of directory if it's now empty
        let rp_path = rk_path.parent()
            // by construction, RK has a parent, its RP
            .unwrap();

        let maybe_first_remaining_rk = syscall!(self.crypto.read_dir_first(
            StorageLocation::Internal,
            rp_path.clone(),
            None,
        )).entry;

        if maybe_first_remaining_rk.is_none() {
            hprintln!("deleting parent {:?} as this was its last RK",
                      &rp_path).ok();
            syscall!(self.crypto.remove_dir(
                StorageLocation::Internal,
                rp_path,
            ));
        } else {
            hprintln!("not deleting deleting parent {:?} as there is {:?}",
                      &rp_path,
                      &maybe_first_remaining_rk.unwrap().path(),
                      ).ok();
        }
        // just return OK
        let response: ctap2::credential_management::Response = Default::default();
        Ok(response)
    }
}

// pub fn get_creds_metadata<S, UP>(
//     authnr: &mut Authenticator<UP>,
// ) -> Result<Response>

// where
//     S: CryptoSyscall,
//     UP: UserPresence
// {

//     let mut response: ctap2::credential_management::Response = Default::default();

//     response.max_possible_remaining_residential_credentials_count =
//         Some(self.state.persistent.max_resident_credentials_guesstimate());

//     // count number of existing RKs
//     todo!();

//     Ok(response)
// }

// pub fn enumerate_rps_begin<S, UP>(
//     authnr: &mut Authenticator<UP>,
// ) -> Result<Response>

// where
//     S: CryptoSyscall,
//     UP: UserPresence
// {

//     let mut response: ctap2::credential_management::Response = Default::default();
//     // rp (0x03): PublicKeyCredentialRpEntity
//     // rpIDHash (0x04) : RP ID SHA-256 hash.
//     // totalRPs (0x05) : Total number of RPs present on the authenticator.

//     let dir = PathBuf::from(b"rk");

//     let maybe_first_rp = syscall!(self.crypto.read_dir_first(
//         StorageLocation::Internal,
//         dir,
//         None,
//     )).entry;

//     response.total_rps = Some(match maybe_first_rp {
//         None => {
//             0
//         }
//         _ => {
//             let mut num_rps = 1;
//             loop {
//                 let maybe_next_rp = syscall!(self.crypto.read_dir_next())
//                     .entry;
//                 match maybe_next_rp {
//                     None => break,
//                     _ => num_rps += 1,
//                 }
//             }
//             num_rps
//         }
//     });

//     if let Some(rp) = maybe_first_rp {

//         // load credential and extract rp and rpIdHash
//         let maybe_first_credential = syscall!(self.crypto.read_dir_first(
//             StorageLocation::Internal,
//             PathBuf::from(rp.path()),
//             None
//         )).entry;

//         match maybe_first_credential {
//             None => panic!("chaos! disorder!"),
//             Some(rk_entry) => {
//                 let serialized = syscall!(self.crypto.read_file(
//                     StorageLocation::Internal,
//                     rk_entry.path().into(),
//                 )).data;

//                 let credential = Credential::deserialize(&serialized)
//                     // this may be a confusing error message
//                     .map_err(|_| Error::InvalidCredential)?;

//                 let rp = credential.data.rp;

//                 response.rp_id_hash = Some(self.hash(&rp.id.as_ref())?);
//                 response.rp = Some(rp);

//             }
//         }

//         // cache state for next call
//         if let Some(total_rps) = response.total_rps {
//             if total_rps > 1 {
//                 let rp_id_hash = response.rp_id_hash.as_ref().unwrap().clone();
//                 self.state.runtime.cache = Some(CommandCache::
//                     CredentialManagementEnumerateRps(total_rps - 1, rp_id_hash));
//             }
//         }
//     }

//     Ok(response)
// }

// pub fn enumerate_rps_get_next_rp<S, UP>(
//     authnr: &mut Authenticator<UP>,
// ) -> Result<Response>

// where
//     S: CryptoSyscall,
//     UP: UserPresence
// {
//     let (remaining, last_rp_id_hash) = match self.state.runtime.cache {
//         Some(CommandCache::CredentialManagementEnumerateRps(
//                 remaining, ref rp_id_hash)) =>
//             (remaining, rp_id_hash),
//         _ => return Err(Error::InvalidCommand),
//     };

//     let dir = PathBuf::from(b"rk");

//     let mut hex = [b'0'; 16];
//     super::format_hex(&last_rp_id_hash[..8], &mut hex);
//     let filename = PathBuf::from(&hex);

//     let maybe_next_rp = syscall!(self.crypto.read_dir_first(
//         StorageLocation::Internal,
//         dir,
//         Some(filename),
//     )).entry;

//     let mut response: ctap2::credential_management::Response = Default::default();

//     if let Some(rp) = maybe_next_rp {
//         // load credential and extract rp and rpIdHash
//         let maybe_first_credential = syscall!(self.crypto.read_dir_first(
//             StorageLocation::Internal,
//             PathBuf::from(rp.path()),
//             None
//         )).entry;

//         match maybe_first_credential {
//             None => panic!("chaos! disorder!"),
//             Some(rk_entry) => {
//                 let serialized = syscall!(self.crypto.read_file(
//                     StorageLocation::Internal,
//                     rk_entry.path().into(),
//                 )).data;

//                 let credential = Credential::deserialize(&serialized)
//                     // this may be a confusing error message
//                     .map_err(|_| Error::InvalidCredential)?;

//                 let rp = credential.data.rp;

//                 response.rp_id_hash = Some(self.hash(&rp.id.as_ref())?);
//                 response.rp = Some(rp);

//                 // cache state for next call
//                 if remaining > 1 {
//                     let rp_id_hash = response.rp_id_hash.as_ref().unwrap().clone();
//                     self.state.runtime.cache = Some(CommandCache::
//                         CredentialManagementEnumerateRps(
//                             remaining - 1, rp_id_hash));
//                 } else {
//                     self.state.runtime.cache = None;

//                 }
//             }
//         }
//     } else {
//         self.state.runtime.cache = None;
//     }

//     Ok(response)
// }

// pub fn delete_credential<S, UP>(
//     authnr: &mut Authenticator<UP>,
//     credential_descriptor: &PublicKeyCredentialDescriptor,
// ) -> Result<Response>

// where
//     S: CryptoSyscall,
//     UP: UserPresence
// {
//     let credential_id_hash = self.hash(&credential_descriptor.id[..])?;
//     let mut hex = [b'0'; 16];
//     super::format_hex(&credential_id_hash[..8], &mut hex);
//     let dir = PathBuf::from(b"rk");
//     let filename = PathBuf::from(&hex);

//     let rk_path = syscall!(self.crypto.locate_file(
//         StorageLocation::Internal,
//         Some(dir.clone()),
//         filename,
//     )).path.ok_or(Error::InvalidCredential)?;


//     // DELETE
//     self.delete_resident_key_by_path(&rk_path)?;

//     // get rid of directory if it's now empty
//     let rp_path = rk_path.parent()
//         // by construction, RK has a parent, its RP
//         .unwrap();

//     let maybe_first_remaining_rk = syscall!(self.crypto.read_dir_first(
//         StorageLocation::Internal,
//         rp_path.clone(),
//         None,
//     )).entry;

//     if maybe_first_remaining_rk.is_none() {
//         // hprintln!("deleting parent {:?} as this was its last RK",
//         //           &rp_path).ok();
//         syscall!(self.crypto.remove_dir(
//             StorageLocation::Internal,
//             rp_path,
//         ));
//     }
//     // just return OK
//     let response: ctap2::credential_management::Response = Default::default();
//     Ok(response)
// }

