//! secp256k1 signing for Ethereum.
//!
//! ECDSA over secp256k1 with RFC-6979 deterministic nonces and low-S
//! normalization (k256 enforces both):
//!
//!   * Ethereum: `Keccak-256(message)` → 65-byte `r || s || v`, where `v`
//!     is the 0/1 recovery id (the host adds the chain offset per EIP-155).
//!
//! `message` is the already-prepared signing payload from the host (e.g.
//! the Ethereum RLP). The device applies the chain-standard hash, so callers
//! don't pre-hash.

use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};

/// Sign a 32-byte digest with secp256k1 (RFC-6979, low-S) → `r(32) || s(32) ||
/// recovery_id(1)`.
pub fn sign_prehash(priv_key: &[u8; 32], digest: &[u8; 32]) -> Option<[u8; 65]> {
    let sk = SigningKey::from_slice(priv_key).ok()?;
    let (sig, recid) = sk.sign_prehash_recoverable(digest).ok()?;
    let mut out = [0u8; 65];
    out[..64].copy_from_slice(&sig.to_bytes());
    out[64] = recid.to_byte();
    Some(out)
}

/// Ethereum: `Keccak-256(message)` → 65-byte `r(32) || s(32) || v(1)`.
pub fn eth_sign(priv_key: &[u8; 32], message: &[u8]) -> Option<[u8; 65]> {
    let digest: [u8; 32] = Keccak256::digest(message).into();
    sign_prehash(priv_key, &digest)
}
