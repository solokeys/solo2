//! # Solo 2 provisioner app
//!
//! Allows injecting arbitrary binary files at arbitrary paths via CCID APDU interface.
//!
//! Used to provision FIDO batch attestation keys and certificates, and to generate
//! Trussed device attestation keys for factory provisioning.
//! See `solo2-cli app provision` for usage.
#![no_std]

use core::convert::TryFrom;

use littlefs2::driver::Storage as LfsStorage;
use littlefs2::path::PathBuf;

use apdu_dispatch::app::{self, CommandView, Interface, VecView};
use defmt::info;
use heapless::Vec;
use iso7816::{Instruction, Status};
use trussed::{
    Client as TrussedClient,
    store::{self, Store},
};
use trussed_core::{syscall, types::Location};

use lpc55_hal as hal;

// Trussed key serialization: flags(2 BE) + kind(2 BE) + material(32)
// Flags: LOCAL=0x0001, SENSITIVE=0x0002
// Kind codes (from trussed src/key.rs Kind::code()):
const KEY_FLAGS_LOCAL_SENSITIVE: u16 = 0x0003; // LOCAL | SENSITIVE
const KEY_KIND_ED255: u16 = 4;
const KEY_KIND_P256: u16 = 5;
const KEY_KIND_X255: u16 = 6;

const SOLO_PROVISIONER_AID: [u8; 9] = [0xA0, 0x00, 0x00, 0x08, 0x47, 0x01, 0x00, 0x00, 0x01];

const TESTER_FILENAME_ID: [u8; 2] = [0xe1, 0x01];
const TESTER_FILE_ID: [u8; 2] = [0xe1, 0x02];

const FILENAME_T1_PUBLIC: &[u8] = b"/attn/pub/00";
const FILENAME_P256_SECRET: &[u8] = b"/attn/sec/01";
const FILENAME_ED255_SECRET: &[u8] = b"/attn/sec/02";
const FILENAME_X255_SECRET: &[u8] = b"/attn/sec/03";
const FILENAME_P256_CERT: &[u8] = b"/attn/x5c/01";
const FILENAME_ED255_CERT: &[u8] = b"/attn/x5c/02";
const FILENAME_X255_CERT: &[u8] = b"/attn/x5c/03";

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Instructions {
    WriteFile = 0xbf,
    BootToBootrom = 0x51,
    ReformatFilesystem = 0xbd,
    GetUuid = 0x62,
    GenerateP256Key = 0xbc,
    GenerateEd255Key = 0xbb,
    GenerateX255Key = 0xb7,
    SaveP256AttestationCertificate = 0xba,
    SaveEd255AttestationCertificate = 0xb9,
    SaveX255AttestationCertificate = 0xb6,
    SaveT1IntermediatePublicKey = 0xb5,
    #[cfg(feature = "test-attestation")]
    TestAttestation = 0xb8,
}

impl TryFrom<u8> for Instructions {
    type Error = ();
    fn try_from(ins: u8) -> core::result::Result<Self, Self::Error> {
        use Instructions::*;
        Ok(match ins {
            0xbf => WriteFile,
            0x51 => BootToBootrom,
            0xbd => ReformatFilesystem,
            0x62 => GetUuid,
            0xbc => GenerateP256Key,
            0xbb => GenerateEd255Key,
            0xb7 => GenerateX255Key,
            0xba => SaveP256AttestationCertificate,
            0xb9 => SaveEd255AttestationCertificate,
            0xb6 => SaveX255AttestationCertificate,
            0xb5 => SaveT1IntermediatePublicKey,
            #[cfg(feature = "test-attestation")]
            0xb8 => TestAttestation,
            _ => return Err(()),
        })
    }
}

enum SelectedBuffer {
    Filename,
    File,
}

pub struct Provisioner<S, FS, T>
where
    S: Store,
    FS: 'static + LfsStorage,
    T: TrussedClient,
{
    trussed: T,

    selected_buffer: SelectedBuffer,
    buffer_filename: Vec<u8, 128>,
    buffer_file_contents: Vec<u8, 8192>,

    store: S,
    stolen_filesystem: &'static mut FS,
    #[allow(dead_code)]
    is_passive: bool,
}

impl<S, FS, T> Provisioner<S, FS, T>
where
    S: Store,
    FS: 'static + LfsStorage,
    T: TrussedClient,
{
    pub fn new(
        trussed: T,
        store: S,
        stolen_filesystem: &'static mut FS,
        is_passive: bool,
    ) -> Provisioner<S, FS, T> {
        Self {
            trussed,
            selected_buffer: SelectedBuffer::Filename,
            buffer_filename: Vec::new(),
            buffer_file_contents: Vec::new(),
            store,
            stolen_filesystem,
            is_passive,
        }
    }

    /// Build a serialized trussed secret key: flags(2 BE) + kind(2 BE) + material(32)
    fn serialize_secret_key(kind_code: u16, material: &[u8; 32]) -> heapless::Vec<u8, 36> {
        let mut bytes: heapless::Vec<u8, 36> = heapless::Vec::new();
        bytes
            .extend_from_slice(&KEY_FLAGS_LOCAL_SENSITIVE.to_be_bytes())
            .unwrap();
        bytes.extend_from_slice(&kind_code.to_be_bytes()).unwrap();
        bytes.extend_from_slice(material).unwrap();
        bytes
    }

    fn handle(&mut self, command: CommandView<'_>, reply: &mut VecView<u8>) -> app::Result {
        match command.instruction() {
            Instruction::Select => self.do_select(command, reply),
            Instruction::WriteBinary => {
                match self.selected_buffer {
                    SelectedBuffer::Filename => self
                        .buffer_filename
                        .extend_from_slice(command.data())
                        .unwrap(),
                    SelectedBuffer::File => self
                        .buffer_file_contents
                        .extend_from_slice(command.data())
                        .unwrap(),
                };
                Ok(())
            }
            Instruction::Unknown(ins) => {
                if let Ok(instruction) = Instructions::try_from(ins) {
                    use Instructions::*;
                    match instruction {
                        ReformatFilesystem => {
                            info!("Reformatting FS");
                            littlefs2::fs::Filesystem::format(self.stolen_filesystem)
                                .map_err(|_| Status::NotEnoughMemory)?;
                            Ok(())
                        }
                        WriteFile => {
                            if self.buffer_file_contents.is_empty()
                                || self.buffer_filename.is_empty()
                            {
                                return Err(Status::IncorrectDataParameter);
                            }
                            let _name = unsafe {
                                core::str::from_utf8_unchecked(self.buffer_filename.as_slice())
                            };
                            info!(
                                "write-file {} ({} bytes)",
                                _name,
                                self.buffer_file_contents.len()
                            );
                            let path = PathBuf::try_from(self.buffer_filename.as_slice())
                                .map_err(|_| Status::IncorrectDataParameter)?;
                            let result = store::store(
                                &self.store,
                                Location::Internal,
                                &path,
                                &self.buffer_file_contents,
                            );
                            self.buffer_file_contents.clear();
                            self.buffer_filename.clear();
                            result.map_err(|_| Status::NotEnoughMemory)
                        }
                        GenerateP256Key => {
                            info!("GenerateP256Key");
                            let mut seed = [0u8; 32];
                            seed.copy_from_slice(
                                syscall!(self.trussed.random_bytes(32)).bytes.as_slice(),
                            );

                            let key_bytes = Self::serialize_secret_key(KEY_KIND_P256, &seed);
                            let path = PathBuf::try_from(FILENAME_P256_SECRET)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            store::store(&self.store, Location::Internal, &path, &key_bytes)
                                .map_err(|_| Status::NotEnoughMemory)?;

                            // Derive public key — pure-Rust p256 (no C bindings)
                            let scalar = p256::Scalar::from_bytes_reduced(
                                p256::FieldBytes::from_slice(&seed),
                            );
                            let affine = p256::AffinePoint::from(
                                p256::ProjectivePoint::generator() * scalar,
                            );
                            let encoded: p256::EncodedPoint = affine.into();
                            // encoded = 0x04 || x(32) || y(32); skip prefix → 64 bytes
                            reply.extend_from_slice(&encoded.as_bytes()[1..]).unwrap();
                            Ok(())
                        }
                        GenerateEd255Key => {
                            info!("GenerateEd255Key");
                            let mut seed = [0u8; 32];
                            seed.copy_from_slice(
                                syscall!(self.trussed.random_bytes(32)).bytes.as_slice(),
                            );

                            let key_bytes = Self::serialize_secret_key(KEY_KIND_ED255, &seed);
                            let path = PathBuf::try_from(FILENAME_ED255_SECRET)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            store::store(&self.store, Location::Internal, &path, &key_bytes)
                                .map_err(|_| Status::NotEnoughMemory)?;

                            let keypair = salty::Keypair::from(&seed);
                            reply.extend_from_slice(keypair.public.as_bytes()).unwrap();
                            Ok(())
                        }
                        GenerateX255Key => {
                            info!("GenerateX255Key");
                            let mut seed = [0u8; 32];
                            seed.copy_from_slice(
                                syscall!(self.trussed.random_bytes(32)).bytes.as_slice(),
                            );

                            let key_bytes = Self::serialize_secret_key(KEY_KIND_X255, &seed);
                            let path = PathBuf::try_from(FILENAME_X255_SECRET)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            store::store(&self.store, Location::Internal, &path, &key_bytes)
                                .map_err(|_| Status::NotEnoughMemory)?;

                            let sk = salty::agreement::SecretKey::from_seed(&seed);
                            let pk = salty::agreement::PublicKey::from(&sk);
                            reply.extend_from_slice(&pk.to_bytes()).unwrap();
                            Ok(())
                        }
                        SaveP256AttestationCertificate => {
                            let secret_path = PathBuf::try_from(FILENAME_P256_SECRET)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            if !store::exists(&self.store, Location::Internal, &secret_path) {
                                return Err(Status::IncorrectDataParameter);
                            }
                            if command.data().len() < 100 {
                                return Err(Status::IncorrectDataParameter);
                            }
                            info!("saving P256 CERT, {} bytes", command.data().len());
                            let cert_path = PathBuf::try_from(FILENAME_P256_CERT)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            store::store(
                                &self.store,
                                Location::Internal,
                                &cert_path,
                                command.data(),
                            )
                            .map_err(|_| Status::NotEnoughMemory)
                        }
                        SaveEd255AttestationCertificate => {
                            let secret_path = PathBuf::try_from(FILENAME_ED255_SECRET)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            if !store::exists(&self.store, Location::Internal, &secret_path) {
                                return Err(Status::IncorrectDataParameter);
                            }
                            if command.data().len() < 100 {
                                return Err(Status::IncorrectDataParameter);
                            }
                            info!("saving ED255 CERT, {} bytes", command.data().len());
                            let cert_path = PathBuf::try_from(FILENAME_ED255_CERT)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            store::store(
                                &self.store,
                                Location::Internal,
                                &cert_path,
                                command.data(),
                            )
                            .map_err(|_| Status::NotEnoughMemory)
                        }
                        SaveX255AttestationCertificate => {
                            let secret_path = PathBuf::try_from(FILENAME_X255_SECRET)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            if !store::exists(&self.store, Location::Internal, &secret_path) {
                                return Err(Status::IncorrectDataParameter);
                            }
                            if command.data().len() < 100 {
                                return Err(Status::IncorrectDataParameter);
                            }
                            info!("saving X255 CERT, {} bytes", command.data().len());
                            let cert_path = PathBuf::try_from(FILENAME_X255_CERT)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            store::store(
                                &self.store,
                                Location::Internal,
                                &cert_path,
                                command.data(),
                            )
                            .map_err(|_| Status::NotEnoughMemory)
                        }
                        SaveT1IntermediatePublicKey => {
                            info!(
                                "saving T1 INTERMEDIATE PUBLIC KEY, {} bytes",
                                command.data().len()
                            );
                            let public_key = command.data();
                            if public_key.len() != 32 {
                                return Err(Status::IncorrectDataParameter);
                            }
                            // Ed255 public key: no SENSITIVE flag
                            let mut key_bytes: heapless::Vec<u8, 36> = heapless::Vec::new();
                            key_bytes
                                .extend_from_slice(&0x0000u16.to_be_bytes())
                                .unwrap();
                            key_bytes
                                .extend_from_slice(&KEY_KIND_ED255.to_be_bytes())
                                .unwrap();
                            key_bytes.extend_from_slice(public_key).unwrap();
                            let path = PathBuf::try_from(FILENAME_T1_PUBLIC)
                                .map_err(|_| Status::UnspecifiedNonpersistentExecutionError)?;
                            store::store(&self.store, Location::Internal, &path, &key_bytes)
                                .map_err(|_| Status::NotEnoughMemory)
                        }
                        GetUuid => {
                            reply.extend_from_slice(&hal::uuid()).unwrap();
                            Ok(())
                        }
                        BootToBootrom => {
                            use hal::traits::flash::WriteErase;
                            let flash = unsafe { hal::peripherals::flash::Flash::steal() }
                                .enabled(&mut unsafe { hal::peripherals::syscon::Syscon::steal() });
                            hal::drivers::flash::FlashGordon::new(flash)
                                .erase_page(0)
                                .ok();
                            hal::raw::SCB::sys_reset()
                        }
                        #[cfg(feature = "test-attestation")]
                        TestAttestation => {
                            // Not implemented in this port
                            Err(Status::FunctionNotSupported)
                        }
                    }
                } else {
                    Err(Status::FunctionNotSupported)
                }
            }
            _ => Err(Status::FunctionNotSupported),
        }
    }

    fn do_select(&mut self, command: CommandView<'_>, _reply: &mut VecView<u8>) -> app::Result {
        if command.data().starts_with(&TESTER_FILENAME_ID) {
            info!("select: filename buffer");
            self.selected_buffer = SelectedBuffer::Filename;
            Ok(())
        } else if command.data().starts_with(&TESTER_FILE_ID) {
            info!("select: file buffer");
            self.selected_buffer = SelectedBuffer::File;
            Ok(())
        } else {
            info!("select: unknown ID");
            Err(Status::NotFound)
        }
    }
}

impl<S, FS, T> iso7816::App for Provisioner<S, FS, T>
where
    S: Store,
    FS: 'static + LfsStorage,
    T: TrussedClient,
{
    fn aid(&self) -> iso7816::Aid {
        iso7816::Aid::new(&SOLO_PROVISIONER_AID)
    }
}

impl<S, FS, T> app::App for Provisioner<S, FS, T>
where
    S: Store,
    FS: 'static + LfsStorage,
    T: TrussedClient,
{
    fn select(
        &mut self,
        _interface: Interface,
        _apdu: CommandView<'_>,
        reply: &mut VecView<u8>,
    ) -> app::Result {
        self.buffer_file_contents.clear();
        self.buffer_filename.clear();
        reply.extend_from_slice(&hal::uuid()).unwrap();
        Ok(())
    }

    fn deselect(&mut self) {}

    fn call(
        &mut self,
        _interface: Interface,
        apdu: CommandView<'_>,
        reply: &mut VecView<u8>,
    ) -> app::Result {
        self.handle(apdu, reply)
    }
}
