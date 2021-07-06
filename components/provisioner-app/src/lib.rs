//! # Solo 2 provisioner app
//!
//! This is a highly *non-portable* Trussed app.
//!
//! It allows injecting arbitrary binary files at arbitrary paths, e.g., to inject FIDO batch
//! attestation keys.
//! It allows generating Trussed device attestation keys and obtaining their public keys,
//! to then generate and inject attn certs from a given root or intermedidate CA.
//!
//! See `solo2-cli` for usage.
#![no_std]

#[macro_use]
extern crate delog;
generate_macros!();

use core::convert::TryFrom;

use trussed::types::LfsStorage;

pub const FILESYSTEM_BOUNDARY: usize = 0x8_0000;

use littlefs2::path::{PathBuf};
use trussed::store::{self, Store};
use trussed::{
    syscall,
    client,
    Client as TrussedClient,
    key::{Kind as KeyKind, Key, Flags},
};
use heapless::Vec;
use apdu_dispatch::iso7816::{Status, Instruction};
use apdu_dispatch::app::Result as ResponseResult;
use apdu_dispatch::{Command, response, command::SIZE as CommandSize, response::SIZE as ResponseSize};

use lpc55_hal as hal;

//
const SOLO_PROVISIONER_AID: [u8; 9] = [ 0xA0, 0x00, 0x00, 0x08, 0x47, 0x01, 0x00, 0x00, 0x01];

const TESTER_FILENAME_ID: [u8; 2] = [0xe1,0x01];
const TESTER_FILE_ID: [u8; 2] = [0xe1,0x02];

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

#[cfg(feature = "test-attestation")]
#[derive(Copy,Clone)]
enum TestAttestationP1 {
    P256Sign = 0,
    P256Cert = 1,
    Ed255Sign= 2,
    Ed255Cert= 3,
    X255Agree = 4,
    X255Cert = 5,
}


const FILENAME_T1_PUBLIC: &'static [u8] = b"/attn/pub/00";

const FILENAME_P256_SECRET: &'static [u8] = b"/attn/sec/01";
const FILENAME_ED255_SECRET: &'static [u8] = b"/attn/sec/02";
const FILENAME_X255_SECRET: &'static [u8] = b"/attn/sec/03";

const FILENAME_P256_CERT: &'static [u8] = b"/attn/x5c/01";
const FILENAME_ED255_CERT: &'static [u8] = b"/attn/x5c/02";
const FILENAME_X255_CERT: &'static [u8] = b"/attn/x5c/03";



enum SelectedBuffer {
    Filename,
    File,
}

pub struct Provisioner<S, FS, T>
where S: Store,
      FS: 'static + LfsStorage,
      T: TrussedClient + client::X255 + client::HmacSha256,
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
where S: Store,
      FS: 'static + LfsStorage,
      T: TrussedClient + client::X255 + client::HmacSha256,
{
    pub fn new(
        trussed: T,
        store: S,
        stolen_filesystem: &'static mut FS,
        is_passive: bool,
    ) -> Provisioner<S, FS, T> {


        return Self {
            trussed,

            selected_buffer: SelectedBuffer::Filename,
            buffer_filename: Vec::new(),
            buffer_file_contents: Vec::new(),
            store,
            stolen_filesystem,
            is_passive,
        }
    }

    fn handle(&mut self, command: &Command, reply: &mut response::Data) -> ResponseResult {

        match command.instruction() {
            Instruction::Select => self.select(command, reply),
            Instruction::WriteBinary => {
                let _offset: u16 = ((command.p1 as u16) << 8) | command.p2 as u16;
                match self.selected_buffer {
                    SelectedBuffer::Filename => self.buffer_filename.extend_from_slice(command.data()).unwrap(),
                    SelectedBuffer::File => self.buffer_file_contents.extend_from_slice(command.data()).unwrap(),
                };
                Ok(())
            }
            Instruction::Unknown(ins) => {
                if let Ok(instruction) = Instructions::try_from(ins) {
                    use Instructions::*;
                    match instruction {
                        ReformatFilesystem => {
                            // Provide a method to reset the FS.
                            info!("Reformatting the FS..");
                            littlefs2::fs::Filesystem::format(self.stolen_filesystem)
                                .map_err(|_| Status::NotEnoughMemory)?;
                            Ok(())
                        }
                        WriteFile => {
                            if self.buffer_file_contents.len() == 0 || self.buffer_filename.len() == 0 {
                                Err(Status::IncorrectDataParameter)
                            } else {
                                // self.buffer_filename.push(0);
                                let _filename = unsafe{ core::str::from_utf8_unchecked(self.buffer_filename.as_slice()) };
                                info!("writing file {} {} bytes", _filename, self.buffer_file_contents.len());
                                // logging::dump_hex(&self.buffer_file_contents, self.buffer_file_contents.len());

                                let res = store::store(
                                    self.store,
                                    trussed::types::Location::Internal,
                                    &PathBuf::from(self.buffer_filename.as_slice()),
                                    &self.buffer_file_contents
                                );
                                self.buffer_file_contents.clear();
                                self.buffer_filename.clear();
                                if !res.is_ok() {
                                    info!("failed writing file!");
                                    Err(Status::NotEnoughMemory)
                                } else {
                                    info!("wrote file");
                                    Ok(())
                                }
                            }
                        }
                        GenerateP256Key => {
                            info!("GenerateP256Key");
                            let mut seed = [0u8; 32];
                            seed.copy_from_slice(
                                &syscall!(self.trussed.random_bytes(32)).bytes.as_slice()
                            );

                            let serialized_key = Key {
                                flags: Flags::LOCAL | Flags::SENSITIVE,
                                kind: KeyKind::P256,
                                material: Vec::from_slice(&seed).unwrap(),
                            };

                            let serialized_bytes = serialized_key.serialize();

                            store::store(
                                self.store,
                                trussed::types::Location::Internal,
                                &PathBuf::from(FILENAME_P256_SECRET),
                                &serialized_bytes
                            ).map_err(|_| Status::NotEnoughMemory)?;
                            info!("stored to {}", core::str::from_utf8(FILENAME_P256_SECRET).unwrap());

                            let keypair = nisty::Keypair::generate_patiently(&seed);

                            reply.extend_from_slice(keypair.public.as_bytes()).unwrap();
                            Ok(())
                        }
                        GenerateEd255Key => {

                            info!("GenerateEd255Key");
                            let mut seed = [0u8; 32];
                            seed.copy_from_slice(
                                &syscall!(self.trussed.random_bytes(32)).bytes.as_slice()
                            );

                            let serialized_key = Key {
                                flags: Flags::LOCAL | Flags::SENSITIVE,
                                kind: KeyKind::Ed255,
                                material: Vec::from_slice(&seed).unwrap(),
                            };

                            // let serialized_key = Key::try_deserialize(&seed[..])
                                // .map_err(|_| Status::WrongLength)?;

                            let serialized_bytes = serialized_key.serialize();

                            store::store(
                                self.store,
                                trussed::types::Location::Internal,
                                &PathBuf::from(FILENAME_ED255_SECRET),
                                &serialized_bytes
                            ).map_err(|_| Status::NotEnoughMemory)?;

                            let keypair = salty::Keypair::from(&seed);

                            reply.extend_from_slice(keypair.public.as_bytes()).unwrap();
                            Ok(())
                        },

                        GenerateX255Key => {

                            info_now!("GenerateX255Key");
                            let mut seed = [0u8; 32];
                            seed.copy_from_slice(
                                &syscall!(self.trussed.random_bytes(32)).bytes.as_slice()
                            );

                            let serialized_key = Key {
                                flags: Flags::LOCAL | Flags::SENSITIVE,
                                kind: KeyKind::X255,
                                material: Vec::from_slice(&seed).unwrap(),
                            };

                            // let serialized_key = Key::try_deserialize(&seed[..])
                                // .map_err(|_| Status::WrongLength)?;

                            let serialized_bytes = serialized_key.serialize();

                            store::store(
                                self.store,
                                trussed::types::Location::Internal,
                                &PathBuf::from(FILENAME_X255_SECRET),
                                &serialized_bytes
                            ).map_err(|_| Status::NotEnoughMemory)?;

                            let secret_key = salty::agreement::SecretKey::from_seed(&seed);
                            let public_key = salty::agreement::PublicKey::from(&secret_key);

                            reply.extend_from_slice(&public_key.to_bytes()).unwrap();
                            Ok(())
                        },

                        SaveP256AttestationCertificate => {
                            let secret_path = PathBuf::from(FILENAME_P256_SECRET);
                            if !secret_path.exists(&self.store.ifs()) {
                                Err(Status::IncorrectDataParameter)
                            } else if command.data().len() < 100 {
                                // Assuming certs will always be >100 bytes
                                Err(Status::IncorrectDataParameter)
                            } else {
                                info!("saving P256 CERT, {} bytes", command.data().len());
                                store::store(
                                    self.store,
                                    trussed::types::Location::Internal,
                                    &PathBuf::from(FILENAME_P256_CERT),
                                    command.data()
                                ).map_err(|_| Status::NotEnoughMemory)?;
                                Ok(())
                            }
                        },

                        SaveEd255AttestationCertificate => {
                            let secret_path = PathBuf::from(FILENAME_ED255_SECRET);
                            if !secret_path.exists(&self.store.ifs()) {
                                Err(Status::IncorrectDataParameter)
                            } else if command.data().len() < 100 {
                                // Assuming certs will always be >100 bytes
                                Err(Status::IncorrectDataParameter)
                            } else {
                                info!("saving ED25519 CERT, {} bytes", command.data().len());
                                store::store(
                                    self.store,
                                    trussed::types::Location::Internal,
                                    &PathBuf::from(FILENAME_ED255_CERT),
                                    command.data()
                                ).map_err(|_| Status::NotEnoughMemory)?;
                                Ok(())
                            }
                        },

                        SaveX255AttestationCertificate => {
                            let secret_path = PathBuf::from(FILENAME_X255_SECRET);
                            if !secret_path.exists(&self.store.ifs()) {
                                Err(Status::IncorrectDataParameter)
                            } else if command.data().len() < 100 {
                                // Assuming certs will always be >100 bytes
                                Err(Status::IncorrectDataParameter)
                            } else {
                                info!("saving X25519 CERT, {} bytes", command.data().len());
                                store::store(
                                    self.store,
                                    trussed::types::Location::Internal,
                                    &PathBuf::from(FILENAME_X255_CERT),
                                    command.data()
                                ).map_err(|_| Status::NotEnoughMemory)?;
                                Ok(())
                            }
                        },

                        SaveT1IntermediatePublicKey => {
                            info!("saving T1 INTERMEDIATE PUBLIC KEY, {} bytes", command.data().len());
                            let public_key = &command.data();
                            if public_key.len() != 32 {
                                Err(Status::IncorrectDataParameter)
                            } else {
                                let serialized_key = Key {
                                    flags: Default::default(),
                                    kind: KeyKind::Ed255,
                                    material: Vec::from_slice(&public_key).unwrap(),
                                };

                                let serialized_key = serialized_key.serialize();

                                store::store(
                                    self.store,
                                    trussed::types::Location::Internal,
                                    &PathBuf::from(FILENAME_T1_PUBLIC),
                                    &serialized_key,
                                ).map_err(|_| Status::NotEnoughMemory)
                            }
                        },

                        #[cfg(feature = "test-attestation")]
                        TestAttestation => {
                            // This is only exposed for development and testing.

                            use trussed::{
                                types::Mechanism,
                                types::SignatureSerialization,
                                types::KeyId,
                                types::Message,
                                types::StorageAttributes,
                                types::Location,
                                types::KeySerialization,

                            };
                            use trussed::config::MAX_SIGNATURE_LENGTH;


                            let p1 = command.p1;
                            let mut challenge = [0u8; 32];
                            challenge.copy_from_slice(
                                &syscall!(self.trussed.random_bytes(32)).bytes.as_slice()
                            );

                            match p1 {
                                _x if p1 == TestAttestationP1::P256Sign as u8 => {
                                    let sig: Vec<MAX_SIGNATURE_LENGTH> = syscall!(self.trussed.sign(
                                        Mechanism::P256,
                                        KeyId::from_special(1),
                                        &challenge,
                                        SignatureSerialization::Asn1Der
                                    )).signature;

                                    // let sig = Bytes::try_from_slice(&sig);

                                    reply.extend_from_slice(&challenge).unwrap();
                                    reply.extend_from_slice(&sig).unwrap();
                                    Ok(())
                                }
                                _x if p1 == TestAttestationP1::P256Cert as u8 => {
                                    let cert: Message = store::read(self.store,
                                        trussed::types::Location::Internal,
                                        &PathBuf::from(FILENAME_P256_CERT),
                                    ).map_err(|_| Status::NotFound)?;
                                    reply.extend_from_slice(&cert).unwrap();
                                    Ok(())
                                }
                                _x if p1 == TestAttestationP1::Ed255Sign as u8 => {

                                    let sig: Vec<MAX_SIGNATURE_LENGTH> = syscall!(self.trussed.sign(
                                        Mechanism::Ed255,
                                        KeyId::from_special(2),
                                        &challenge,
                                        SignatureSerialization::Asn1Der
                                    )).signature;

                                    // let sig = Bytes::try_from_slice(&sig);

                                    reply.extend_from_slice(&challenge).unwrap();
                                    reply.extend_from_slice(&sig).unwrap();
                                    Ok(())
                                }
                                _x if p1 == TestAttestationP1::Ed255Cert as u8 => {
                                    let cert:Message = store::read(self.store,
                                        trussed::types::Location::Internal,
                                        &PathBuf::from(FILENAME_ED255_CERT),
                                    ).map_err(|_| Status::NotFound)?;
                                    reply.extend_from_slice(&cert).unwrap();
                                    Ok(())
                                }
                                _x if p1 == TestAttestationP1::X255Agree as u8 => {

                                    syscall!(self.trussed.debug_dump_store());

                                    let mut platform_pk_bytes = [0u8; 32];
                                    for i in 0 .. 32 {
                                        platform_pk_bytes[i] = command.data()[i]
                                    }

                                    info_now!("1");

                                    let platform_kak = syscall!(self.trussed.deserialize_key(
                                        Mechanism::X255,
                                        // platform sends it's pk as 32 bytes
                                        &platform_pk_bytes,
                                        KeySerialization::Raw,
                                        StorageAttributes::new().set_persistence(Location::Volatile)
                                    )).key;
                                    info_now!("3");

                                    let shared_secret = syscall!(self.trussed.agree_x255(
                                        KeyId::from_special(3),
                                        platform_kak,
                                        Location::Volatile
                                    )).shared_secret;
                                    info_now!("4");

                                    let sig = syscall!(self.trussed.sign_hmacsha256(
                                        shared_secret,
                                        &challenge,
                                    )).signature;

                                    info_now!("5");
                                    reply.extend_from_slice(&challenge).unwrap();
                                    reply.extend_from_slice(&sig).unwrap();
                                    Ok(())
                                }
                                _x if p1 == TestAttestationP1::X255Cert as u8 => {
                                    let cert: Message = store::read(self.store,
                                        trussed::types::Location::Internal,
                                        &PathBuf::from(FILENAME_X255_CERT),
                                    ).map_err(|_| Status::NotFound)?;
                                    reply.extend_from_slice(&cert).unwrap();
                                    Ok(())
                                }
                                _ => Err(Status::FunctionNotSupported)

                            }

                        }

                        GetUuid => {
                            // Get UUID
                            reply.extend_from_slice(&hal::uuid()).unwrap();
                            Ok(())
                        },
                        BootToBootrom => {
                            // Boot to bootrom via flash 0 page erase
                            use hal::traits::flash::WriteErase;
                            let flash = unsafe { hal::peripherals::flash::Flash::steal() }.enabled(
                                &mut unsafe { hal::peripherals::syscon::Syscon::steal()}
                            );
                            hal::drivers::flash::FlashGordon::new(flash).erase_page(0).ok();
                            hal::raw::SCB::sys_reset()
                        },

                    }
                } else {
                    Err(Status::FunctionNotSupported)
                }
            }
            _ => Err(Status::FunctionNotSupported),
        }
    }

    fn select(&mut self, command: &Command, _reply: &mut response::Data) -> ResponseResult {

        if command.data().starts_with(&TESTER_FILENAME_ID) {
            info!("select filename");
            self.selected_buffer = SelectedBuffer::Filename;
            Ok(())
        } else if command.data().starts_with(&TESTER_FILE_ID) {
            info!("select file");
            self.selected_buffer = SelectedBuffer::File;
            Ok(())
        } else {
            info!("unknown ID: {:?}", &command.data());
            Err(Status::NotFound)
        }

    }

}

impl<S, FS, T> apdu_dispatch::iso7816::App for Provisioner<S, FS, T>
where S: Store,
      FS: 'static + LfsStorage,
      T: TrussedClient + client::X255 + client::HmacSha256,
{
    fn aid(&self) -> apdu_dispatch::iso7816::Aid {
        apdu_dispatch::iso7816::Aid::new(&SOLO_PROVISIONER_AID)
    }
}


impl<S, FS, T> apdu_dispatch::app::App<CommandSize, ResponseSize> for Provisioner<S, FS, T>
where S: Store,
      FS: 'static + LfsStorage,
      T: TrussedClient + client::X255 + client::HmacSha256,
{
    fn select(&mut self, _apdu: &Command, reply: &mut response::Data) -> apdu_dispatch::app::Result {
        self.buffer_file_contents.clear();
        self.buffer_filename.clear();
        // For manufacture speed, return uuid on select
        reply.extend_from_slice(&hal::uuid()).unwrap();
        Ok(())
    }

    fn deselect(&mut self) -> () {
    }

    fn call(&mut self, _interface_type: apdu_dispatch::app::Interface, apdu: &Command, reply: &mut response::Data) -> apdu_dispatch::app::Result {
        self.handle(&apdu, reply)
    }
}
