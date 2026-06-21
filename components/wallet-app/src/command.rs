use crate::{derivation_path::DerivationPath, APDU_CLA};
use core::convert::TryFrom;
use iso7816::Status;

/// Wallet app commands
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command<'l> {
    /// Get application configuration
    GetAppConfiguration,
    /// Get public key for a derivation path
    GetPubkey(GetPubkey<'l>),
    /// Ledger Solana signMessage (INS 0x06). The host chunks the payload across
    /// APDUs via P2 (EXTEND/MORE); the first chunk carries `[numPaths][path]`
    /// then message bytes, continuations carry raw message. P2 is passed through
    /// for the authenticator to drive the chunk state machine.
    SignMessage { p2: u8, chunk: &'l [u8] },
    /// Ledger Solana signOffchainMessage (INS 0x07) — dApp sign-in. Same chunked
    /// wire layout as SignMessage; the message is a full Solana offchain message
    /// (`\xffsolana offchain` domain prefix + header + content), Ed25519-signed
    /// verbatim.
    SignOffchainMessage { p2: u8, chunk: &'l [u8] },
    /// Reset secret to zero private key
    Reset,
    /// Generate a new seed
    Keygen { export: bool },
    /// Set a new seed
    SetSeed(&'l [u8]),
    /// Set a new private key
    SetPrivateKey(&'l [u8]),
    /// Get secret type
    GetSecretType,
    /// Ledger Ethereum app: getAddress (INS 0x02).
    EthGetAddress(EthGetAddress<'l>),
    /// Ledger Ethereum app: getAppConfiguration (INS 0x06 with no data).
    EthGetAppConfiguration,
    /// Ledger Ethereum app: signTransaction (INS 0x04 with data). `first` is
    /// the P1=0x00 first chunk ([pathLen][path][RLP…]); continuations
    /// (P1=0x80) carry more RLP.
    EthSignTransaction { first: bool, chunk: &'l [u8] },
    /// Ledger Ethereum app: signPersonalMessage / EIP-191 (INS 0x08). First
    /// chunk = [pathLen][path][msgLen(4B)][message…]; continuations carry more.
    EthSignPersonalMessage { first: bool, chunk: &'l [u8] },
    /// Set the active chain (INS 0x16): P1 = 0x01 → Ethereum, else Solana.
    SetChain { eth: bool },
}

/// Ledger Ethereum `getAddress` command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EthGetAddress<'l> {
    pub derivation_path: DerivationPath<'l>,
    /// P1 = 0x01 → confirm on device (requires user presence).
    pub display: bool,
    /// P2 = 0x01 → also return the BIP-32 chain code.
    pub chaincode: bool,
}

/// Get public key command
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GetPubkey<'l> {
    /// Parsed derivation path
    pub derivation_path: DerivationPath<'l>,
}

/// Sign message command
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignMessage<'l> {
    /// Parsed derivation path
    pub derivation_path: DerivationPath<'l>,
    /// Message to sign
    pub message: &'l [u8],
}

/// Command instruction codes
mod instructions {
    /// Ledger Ethereum app: getAddress.
    pub const ETH_GET_ADDRESS: u8 = 0x02;
    /// Ledger Ethereum app: signPersonalMessage (EIP-191).
    pub const ETH_SIGN_PERSONAL_MESSAGE: u8 = 0x08;
    pub const GET_APP_CONFIGURATION: u8 = 0x04;
    pub const GET_PUBKEY: u8 = 0x05;
    /// Solana signMessage (with data) / Ethereum getAppConfiguration (no data).
    pub const SIGN_MESSAGE: u8 = 0x06;
    /// Solana signOffchainMessage (dApp sign-in).
    pub const SIGN_OFFCHAIN_MESSAGE: u8 = 0x07;
    pub const RESET: u8 = 0x10;
    pub const KEYGEN: u8 = 0x11;
    pub const SET_SEED: u8 = 0x12;
    pub const SET_PRIVATE_KEY: u8 = 0x13;
    pub const GET_SECRET_TYPE: u8 = 0x14;
    /// Select the active chain (Solana / Ethereum) for the Ledger probe.
    pub const SET_CHAIN: u8 = 0x16;
}

/// Parse a Solana sign payload `[num_paths][path][message…]` (shared by
/// signMessage and signOffchainMessage). We support a single derivation path.
pub fn parse_sign_message(data: &[u8]) -> Result<SignMessage<'_>, Status> {
    let num_paths = *data.first().ok_or(Status::WrongLength)? as usize;
    if num_paths == 0 || num_paths > 10 {
        return Err(Status::IncorrectDataParameter);
    }
    let path_data_start = 1;
    let derivation_path = DerivationPath::parse(&data[path_data_start..])?;
    let path_len = 1 + (derivation_path.depth as usize * 4);
    let message_start = path_data_start + path_len;
    if message_start > data.len() {
        return Err(Status::WrongLength);
    }
    Ok(SignMessage {
        derivation_path,
        message: &data[message_start..],
    })
}

impl<'l, const C: usize> TryFrom<&'l iso7816::Command<C>> for Command<'l> {
    type Error = Status;

    fn try_from(command: &'l iso7816::Command<C>) -> Result<Self, Self::Error> {
        let class = command.class();
        let instruction_byte: u8 = command.instruction().into();
        let p1 = command.p1;
        let p2 = command.p2;
        let data = command.data();

        // Check CLA first (allow both 0xE0 for proprietary and 0x00 for standard ISO7816)
        let cla_byte: u8 = class.into_inner();
        if cla_byte != APDU_CLA && cla_byte != 0x00 {
            return Err(Status::ClassNotSupported);
        }

        // For proprietary CLA (0xE0), secure messaging and channel checks may be different
        // For standard ISO7816 (0x00), check secure messaging and channel (like OATH)
        if cla_byte == 0x00 {
            if !class.secure_messaging().none() {
                return Err(Status::SecureMessagingNotSupported);
            }

            if class.channel() != Some(0) {
                return Err(Status::LogicalChannelNotSupported);
            }
        } else {
            // For proprietary CLA (0xE0), we're more lenient
            // Only reject if secure messaging is explicitly enabled (not just Unknown)
            // Channel can be None for proprietary CLAs
        }

        // Parse commands directly from ISO7816 command fields
        match instruction_byte {
            instructions::GET_APP_CONFIGURATION => {
                // INS 0x04 collides: Solana getAppConfiguration (no data) vs
                // Ethereum signTransaction (carries the path + RLP) — split on Lc.
                if data.is_empty() {
                    if p1 != 0 || p2 != 0 {
                        return Err(Status::IncorrectDataParameter);
                    }
                    Ok(Command::GetAppConfiguration)
                } else {
                    // P1 = 0x00 first chunk, 0x80 continuation.
                    Ok(Command::EthSignTransaction {
                        first: p1 == 0x00,
                        chunk: data.as_slice(),
                    })
                }
            }
            instructions::ETH_SIGN_PERSONAL_MESSAGE => {
                // P1 = 0x00 first chunk, 0x80 continuation.
                Ok(Command::EthSignPersonalMessage {
                    first: p1 == 0x00,
                    chunk: data.as_slice(),
                })
            }
            instructions::ETH_GET_ADDRESS => {
                // Ledger Ethereum getAddress. P1 = display, P2 = chaincode.
                // Data = [pathLen][components...] (+ optional 4-byte chainId,
                // which `DerivationPath::parse` tolerates as trailing bytes).
                let derivation_path = DerivationPath::parse(data.as_slice())?;
                Ok(Command::EthGetAddress(EthGetAddress {
                    derivation_path,
                    display: p1 == 0x01,
                    chaincode: p2 & 0x01 != 0,
                }))
            }
            instructions::GET_PUBKEY => {
                let derivation_path = DerivationPath::parse(data.as_slice())?;
                Ok(Command::GetPubkey(GetPubkey { derivation_path }))
            }
            instructions::SIGN_MESSAGE => {
                // INS 0x06 collides between Solana signMessage (carries data) and
                // Ethereum getAppConfiguration (no data) — split on Lc.
                if data.is_empty() {
                    return Ok(Command::EthGetAppConfiguration);
                }
                Ok(Command::SignMessage {
                    p2,
                    chunk: data.as_slice(),
                })
            }
            instructions::SIGN_OFFCHAIN_MESSAGE => Ok(Command::SignOffchainMessage {
                p2,
                chunk: data.as_slice(),
            }),
            instructions::RESET => {
                if p1 != 0 || p2 != 0 || !data.is_empty() {
                    return Err(Status::IncorrectDataParameter);
                }
                Ok(Command::Reset)
            }
            instructions::KEYGEN => {
                // P1: 0x00 = locked (don't export), 0x01 = exported (show words)
                // P2: reserved, must be 0
                // Data: empty
                if p2 != 0 || !data.is_empty() {
                    return Err(Status::IncorrectDataParameter);
                }
                let export = p1 == 0x01;
                Ok(Command::Keygen { export })
            }
            instructions::SET_SEED => {
                // P1, P2: reserved, must be 0
                // Data: 32 bytes seed
                if p1 != 0 || p2 != 0 {
                    return Err(Status::IncorrectDataParameter);
                }
                let data_slice = data.as_slice();
                if data_slice.len() != 32 {
                    return Err(Status::WrongLength);
                }
                Ok(Command::SetSeed(data_slice))
            }
            instructions::SET_PRIVATE_KEY => {
                // P1, P2: reserved, must be 0
                // Data: 32 bytes private key
                if p1 != 0 || p2 != 0 {
                    return Err(Status::IncorrectDataParameter);
                }
                let data_slice = data.as_slice();
                if data_slice.len() != 32 {
                    return Err(Status::WrongLength);
                }
                Ok(Command::SetPrivateKey(data_slice))
            }
            instructions::GET_SECRET_TYPE => {
                if p1 != 0 || p2 != 0 || !data.is_empty() {
                    return Err(Status::IncorrectDataParameter);
                }
                Ok(Command::GetSecretType)
            }
            instructions::SET_CHAIN => {
                // P1: 0x01 = Ethereum, else Solana.
                Ok(Command::SetChain { eth: p1 == 0x01 })
            }
            _ => Err(Status::InstructionNotSupportedOrInvalid),
        }
    }
}
