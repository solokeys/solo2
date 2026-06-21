//! Chain selection from the BIP-44 `coin_type` in the derivation path.
//!
//! The wire protocol is unchanged — the host sends a standard BIP-44 path
//! (`m/44'/coin'/account'/...`) and the app picks the curve + signing
//! convention from `coin_type` (the second component, with the hardened
//! bit stripped):
//!
//!   * `501` → Solana   (Ed25519, SLIP-10, sign the raw message)
//!   * `60`  → Ethereum (secp256k1, BIP-32, sign Keccak-256(message))
//!
//! Anything else (including a missing component) falls back to Solana, so
//! pre-existing Ed25519 behaviour is preserved.

use crate::derivation_path::DerivationPath;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chain {
    Solana,
    Ethereum,
}

impl Chain {
    /// BIP-44 `coin_type` values (`SLIP-0044`).
    const ETHEREUM_COIN: u32 = 60;

    /// Pick the chain from a derivation path's `coin_type` (component 1,
    /// hardened bit stripped). Defaults to Solana.
    pub fn from_path(path: &DerivationPath) -> Self {
        match path.component(1).map(|c| c & 0x7fff_ffff) {
            Some(Self::ETHEREUM_COIN) => Chain::Ethereum,
            _ => Chain::Solana,
        }
    }
}
