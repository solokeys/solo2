use iso7816::Status;
use littlefs2_core::{path, Path};
use trussed_core::{
    syscall, try_syscall,
    types::{Location, Message, PathBuf},
};

/// Secret type discriminator
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretType {
    /// Empty (0x00) - no secret set, get_pubkey and sign will fail
    Empty,
    /// Private key (0x01) - use directly, ignore path
    PrivateKey,
    /// Imported seed (0x02) - set from CLI seed command, derive from path
    ImportedSeed,
    /// Exported seed (0x03) - keygen without -s, words shown, derive from path
    ExportedSeed,
    /// Locked seed (0x04) - keygen with -s, words not shown, derive from path
    LockedSeed,
}

impl SecretType {
    pub fn to_byte(self) -> u8 {
        match self {
            SecretType::Empty => 0x00,
            SecretType::PrivateKey => 0x01,
            SecretType::ImportedSeed => 0x02,
            SecretType::ExportedSeed => 0x03,
            SecretType::LockedSeed => 0x04,
        }
    }

    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(SecretType::Empty),
            0x01 => Some(SecretType::PrivateKey),
            0x02 => Some(SecretType::ImportedSeed),
            0x03 => Some(SecretType::ExportedSeed),
            0x04 => Some(SecretType::LockedSeed),
            _ => None,
        }
    }

    pub fn is_seed(self) -> bool {
        matches!(
            self,
            SecretType::ImportedSeed | SecretType::ExportedSeed | SecretType::LockedSeed
        )
    }
}

/// Persistent secret state stored in filesystem
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SecretState {
    pub secret_type: u8,
    pub secret_bytes: [u8; 32],
}

/// Runtime state (not persisted)
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Runtime {}

/// State management for the wallet app
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct State {
    pub runtime: Runtime,
}

impl State {
    const FILENAME: &'static Path = path!("secret.bin");

    pub fn persistent<T, X>(
        &mut self,
        trussed: &mut T,
        f: impl FnOnce(&mut T, &mut SecretState) -> X,
    ) -> X
    where
        T: trussed::Client,
    {
        let mut state: SecretState =
            try_syscall!(trussed.read_file(Location::Internal, PathBuf::from(Self::FILENAME)))
                .map(|response| postcard::from_bytes(&response.data).unwrap())
                .unwrap_or(SecretState {
                    secret_type: 0x00,
                    secret_bytes: [0u8; 32],
                });

        let x = f(trussed, &mut state);

        let mut buf = [0u8; 1024];
        let serialized = postcard::to_slice(&state, &mut buf).unwrap();
        syscall!(trussed.write_file(
            Location::Internal,
            PathBuf::from(Self::FILENAME),
            Message::try_from(&*serialized).unwrap(),
            None,
        ));
        x
    }

    pub fn try_persistent<T>(
        &mut self,
        trussed: &mut T,
        f: impl FnOnce(&mut T, &mut SecretState) -> Result<(), Status>,
    ) -> Result<(), Status>
    where
        T: trussed::Client,
    {
        let mut state: SecretState =
            try_syscall!(trussed.read_file(Location::Internal, PathBuf::from(Self::FILENAME)))
                .map(|response| postcard::from_bytes(&response.data).unwrap())
                .map_err(|_| Status::NotFound)?;

        let result = f(trussed, &mut state);

        let mut buf = [0u8; 1024];
        let serialized = postcard::to_slice(&state, &mut buf).unwrap();
        syscall!(trussed.write_file(
            Location::Internal,
            PathBuf::from(Self::FILENAME),
            Message::try_from(&*serialized).unwrap(),
            None,
        ));
        result
    }
}
