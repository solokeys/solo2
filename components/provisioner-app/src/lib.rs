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

use trussed::types::LfsStorage;

pub const FILESYSTEM_BOUNDARY: usize = 0x8_0000;

use littlefs2::path::{PathBuf};
use trussed::store::{self, Store};
use trussed::{
    syscall,
    Client as TrussedClient,
    key::{Kind as KeyKind, Key, Flags},
};
use heapless_bytes::Bytes;
use apdu_dispatch::iso7816::{Status, Instruction};
use apdu_dispatch::app::Result as ResponseResult;
use apdu_dispatch::types::{Command, response, command};

use lpc55_hal as hal;

//
const SOLO_PROVISIONER_AID: [u8; 9] = [ 0xA0, 0x00, 0x00, 0x08, 0x47, 0x01, 0x00, 0x00, 0x01];

const TESTER_FILENAME_ID: [u8; 2] = [0xe1,0x01];
const TESTER_FILE_ID: [u8; 2] = [0xe1,0x02];

const WRITE_FILE_INS: u8 = 0xbf;

const REFORMAT_FS_INS: u8 = 0xbd;
const GET_UUID_INS: u8 = 0x62;

const GENERATE_P256_ATTESTATION: u8 = 0xbc;
const GENERATE_ED255_ATTESTATION: u8 = 0xbb;

const SAVE_P256_ATTESTATION_CERT: u8 = 0xba;
const SAVE_ED255_ATTESTATION_CERT: u8 = 0xb9;
#[cfg(feature = "test-attestation")]
const TEST_ATTESTATION: u8 = 0xb8;

#[cfg(feature = "test-attestation")]
#[derive(Copy,Clone)]
enum TestAttestationP1 {
    P256Sign = 0,
    P256Cert = 1,
    Ed255Sign= 2,
    Ed255Cert= 3,
}


const FILENAME_P256_SECRET: &'static [u8] = b"/attn/sec/01";
const FILENAME_ED255_SECRET: &'static [u8] = b"/attn/sec/02";

const FILENAME_P256_CERT: &'static [u8] = b"/attn/x5c/01";
const FILENAME_ED255_CERT: &'static [u8] = b"/attn/x5c/02";



enum SelectedBuffer {
    Filename,
    File,
}

pub struct Provisioner<S, FS, T>
where S: Store,
      FS: 'static + LfsStorage,
      T: TrussedClient,
{
    trussed: T,

    selected_buffer: SelectedBuffer,
    buffer_filename: Bytes<heapless::consts::U128>,
    buffer_file_contents: Bytes<heapless::consts::U8192>,

    store: S,
    stolen_filesystem: &'static mut FS,
    #[allow(dead_code)]
    is_passive: bool,
}

impl<S, FS, T> Provisioner<S, FS, T>
where S: Store,
      FS: 'static + LfsStorage,
      T: TrussedClient,
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
            buffer_filename: Bytes::new(),
            buffer_file_contents: Bytes::new(),
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
                match ins {
                    REFORMAT_FS_INS => {
                        // Provide a method to reset the FS.
                        info!("Reformatting the FS..");
                        littlefs2::fs::Filesystem::format(self.stolen_filesystem)
                            .map_err(|_| Status::NotEnoughMemory)?;
                        Ok(())
                    }
                    WRITE_FILE_INS => {
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
                    GENERATE_P256_ATTESTATION => {
                        info!("GENERATE_P256_ATTESTATION");
                        let mut seed = [0u8; 32];
                        seed.copy_from_slice(
                            &syscall!(self.trussed.random_bytes(32)).bytes.as_slice()
                        );

                        let serialized_key = Key {
                            flags: Flags::LOCAL | Flags::SENSITIVE,
                            kind: KeyKind::P256,
                            material: Bytes::try_from_slice(&seed).unwrap(),
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
                    GENERATE_ED255_ATTESTATION => {

                        info!("GENERATE_ED255_ATTESTATION");
                        let mut seed = [0u8; 32];
                        seed.copy_from_slice(
                            &syscall!(self.trussed.random_bytes(32)).bytes.as_slice()
                        );

                        let serialized_key = Key {
                            flags: Flags::LOCAL | Flags::SENSITIVE,
                            kind: KeyKind::Ed255,
                            material: Bytes::try_from_slice(&seed).unwrap(),
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

                    SAVE_P256_ATTESTATION_CERT => {
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

                    SAVE_ED255_ATTESTATION_CERT => {
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

                    #[cfg(feature = "test-attestation")]
                    TEST_ATTESTATION => {
                        // This is only exposed for development and testing.

                        use trussed::{
                            types::Mechanism,
                            types::SignatureSerialization,
                            types::ObjectHandle
                        };


                        let p1 = command.p1;
                        let mut challenge = [0u8; 32];
                        challenge.copy_from_slice(
                            &syscall!(self.trussed.random_bytes(32)).bytes.as_slice()
                        );

                        match p1 {
                            _x if p1 == TestAttestationP1::P256Sign as u8 => {
                                let sig: Bytes<MAX_SIGNATURE_LENGTH> = syscall!(self.trussed.sign(
                                    Mechanism::P256,
                                    ObjectHandle{object_id: 1.into()},
                                    &challenge,
                                    SignatureSerialization::Asn1Der
                                )).signature;

                                // let sig = Bytes::try_from_slice(&sig);

                                reply.extend_from_slice(&challenge).unwrap();
                                reply.extend_from_slice(&sig).unwrap();
                                Ok(())
                            }
                            _x if p1 == TestAttestationP1::P256Cert as u8 => {
                                store::read(self.store,
                                    trussed::types::Location::Internal,
                                    &PathBuf::from(FILENAME_P256_CERT),
                                ).map_err(|_| Status::NotFound)
                            }
                            _x if p1 == TestAttestationP1::Ed255Sign as u8 => {

                                let sig: Bytes<MAX_SIGNATURE_LENGTH> = syscall!(self.trussed.sign(
                                    Mechanism::Ed255,
                                    ObjectHandle{object_id: 2.into()},
                                    &challenge,
                                    SignatureSerialization::Asn1Der
                                )).signature;

                                // let sig = Bytes::try_from_slice(&sig);

                                reply.extend_from_slice(&challenge).unwrap();
                                reply.extend_from_slice(&sig).unwrap();
                                Ok(())
                            }
                            _x if p1 == TestAttestationP1::Ed255Cert as u8 => {
                                store::read(self.store,
                                    trussed::types::Location::Internal,
                                    &PathBuf::from(FILENAME_ED255_CERT),
                                ).map_err(|_| Status::NotFound)
                            }
                            _ => Err(Status::FunctionNotSupported)

                        }

                    }

                    GET_UUID_INS => {
                        // Get UUID
                        reply.extend_from_slice(&hal::uuid()).unwrap();
                        Ok(())
                    },
                    0x51 => {
                        // Boot to bootrom via flash 0 page erase
                        use hal::traits::flash::WriteErase;
                        let flash = unsafe { hal::peripherals::flash::Flash::steal() }.enabled(
                            &mut unsafe { hal::peripherals::syscon::Syscon::steal()}
                        );
                        hal::drivers::flash::FlashGordon::new(flash).erase_page(0).ok();
                        hal::raw::SCB::sys_reset()
                    },

                    _ => {
                        Err(Status::FunctionNotSupported)
                    }
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

impl<S, FS, T> apdu_dispatch::app::Aid for Provisioner<S, FS, T>
where S: Store,
      FS: 'static + LfsStorage,
      T: TrussedClient
{

    fn aid(&self) -> &'static [u8] {
        &SOLO_PROVISIONER_AID
    }

    fn right_truncated_length(&self) -> usize {
        9
    }
}


impl<S, FS, T> apdu_dispatch::app::App<command::Size, response::Size> for Provisioner<S, FS, T>
where S: Store,
      FS: 'static + LfsStorage,
      T: TrussedClient,
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
