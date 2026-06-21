//! SLIP-10 Ed25519 derivation from a 32-byte master seed and a BIP-32
//! derivation path.
//!
//! SLIP-10 (Trezor's standard, used by all Solana wallets) derives a
//! per-path 32-byte private key via chained HMAC-SHA512 starting from
//! the master seed. Ed25519 only supports **hardened** derivation
//! (every component must have its top bit set); paths Solana actually
//! uses (`m/44'/501'/...`) are all hardened by construction.
//!
//! Master key: `I = HMAC-SHA512("ed25519 seed", seed)`, `IL = I[0..32]`
//! is the master private key, `IR = I[32..64]` the master chain code.
//!
//! Child key: `I = HMAC-SHA512(parent_chain_code, 0x00 || parent_priv ||
//! index_BE)`, with the same `IL` / `IR` split.
//!
//! The 32-byte private key from the final derivation step is fed
//! verbatim into `salty::Keypair::from(&seed)`, which performs the
//! standard Ed25519 secret-key expansion (SHA-512 then bit clamping).

use crate::derivation_path::DerivationPath;
use crate::state::{SecretState, SecretType};
use hmac::{Hmac, Mac};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::elliptic_curve::PrimeField;
use k256::{FieldBytes, ProjectivePoint, Scalar};
use salty::Keypair;
use sha2::Sha512;

type HmacSha512 = Hmac<Sha512>;

/// Master-seed HMAC key string from the SLIP-10 spec for Ed25519.
const ED25519_HMAC_KEY: &[u8] = b"ed25519 seed";
/// secp256k1 master-seed HMAC key. The literal `"Bitcoin seed"` is the fixed
/// BIP-32 constant for the curve (used for every secp256k1 chain, e.g.
/// Ethereum); the bytes are mandated by the spec, not a chain reference.
const SECP256K1_HMAC_KEY: &[u8] = b"Bitcoin seed";

/// Derive an Ed25519 keypair from `state` and `path`.
///
/// * `Empty` secret type → all-zero keypair (caller should have
///   short-circuited before reaching here).
/// * `PrivateKey` → the 32 bytes are used directly as the Ed25519 seed,
///   ignoring the path. Matches what Solana's `solana-keygen` does for
///   raw imports and what existing on-device PrivateKey records expect.
/// * Any seed type → SLIP-10 derive private key for `path`, then build
///   the keypair from those 32 bytes.
pub fn derive_keypair(state: &SecretState, path: DerivationPath) -> Keypair {
    let secret_type = SecretType::from_byte(state.secret_type).unwrap_or(SecretType::Empty);

    if secret_type == SecretType::PrivateKey {
        return Keypair::from(&state.secret_bytes);
    }
    if secret_type == SecretType::Empty {
        return Keypair::from(&[0u8; 32]);
    }

    let derived = derive_slip10_ed25519(&state.secret_bytes, &path);
    Keypair::from(&derived)
}

/// Derive a secp256k1 private key from `state` and `path`, mirroring
/// [`derive_keypair`]'s secret-type handling:
///
/// * `PrivateKey` → the 32 bytes are the private key verbatim (path ignored).
/// * `Empty` → `None` (caller short-circuits).
/// * Any seed type → BIP-32 derive for `path`.
pub fn derive_secp256k1_priv(state: &SecretState, path: &DerivationPath) -> Option<[u8; 32]> {
    match SecretType::from_byte(state.secret_type).unwrap_or(SecretType::Empty) {
        SecretType::PrivateKey => Some(state.secret_bytes),
        SecretType::Empty => None,
        _ => derive_bip32_secp256k1(&state.secret_bytes, path),
    }
}

/// As [`derive_secp256k1_priv`] but also returns the leaf chain code. A raw
/// `PrivateKey` secret has no BIP-32 chain code, so a zero chain code is
/// returned for it (and the path is ignored, matching `derive_secp256k1_priv`).
pub fn derive_secp256k1_with_chaincode(
    state: &SecretState,
    path: &DerivationPath,
) -> Option<([u8; 32], [u8; 32])> {
    match SecretType::from_byte(state.secret_type).unwrap_or(SecretType::Empty) {
        SecretType::PrivateKey => Some((state.secret_bytes, [0u8; 32])),
        SecretType::Empty => None,
        _ => derive_bip32_secp256k1_with_chaincode(&state.secret_bytes, path),
    }
}

/// Walk the SLIP-10 chain from a 32-byte master seed through the path
/// components, returning the leaf 32-byte Ed25519 private key.
///
/// For Ed25519 every component must be hardened (top bit set). On-wire
/// the components arrive already hardened from the host; this function
/// uses them as-is.
fn derive_slip10_ed25519(seed: &[u8; 32], path: &DerivationPath) -> [u8; 32] {
    // Master key: HMAC-SHA512("ed25519 seed", seed)
    let i = hmac_sha512(ED25519_HMAC_KEY, seed);
    let mut key = copy32(&i[0..32]);
    let mut chain_code = copy32(&i[32..64]);

    for idx in 0..path.depth as usize {
        let component = path.component(idx).unwrap_or(0);
        // CKDpriv: I = HMAC-SHA512(chain_code, 0x00 || key || index_BE)
        let mut data = [0u8; 1 + 32 + 4];
        data[0] = 0x00;
        data[1..33].copy_from_slice(&key);
        data[33..37].copy_from_slice(&component.to_be_bytes());
        let i = hmac_sha512(&chain_code, &data);
        key = copy32(&i[0..32]);
        chain_code = copy32(&i[32..64]);
    }

    key
}

fn hmac_sha512(key: &[u8], data: &[u8]) -> [u8; 64] {
    let mut mac = HmacSha512::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    let out = mac.finalize().into_bytes();
    let mut buf = [0u8; 64];
    buf.copy_from_slice(&out);
    buf
}

fn copy32(slice: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&slice[..32]);
    out
}

// ───── secp256k1 (Ethereum) ──────────────────────────────────────────────────

/// Parse 32 big-endian bytes as a secp256k1 scalar, failing if `>= n`
/// (BIP-32 treats that intermediate as invalid). Zero is allowed here —
/// callers reject a zero *result* where the spec requires it.
fn scalar_lt_n(bytes: &[u8]) -> Option<Scalar> {
    let fb = FieldBytes::clone_from_slice(&bytes[..32]);
    Scalar::from_repr(fb).into()
}

/// SEC1-compressed (33-byte) public key for a private scalar.
fn compressed_pubkey(scalar: &Scalar) -> [u8; 33] {
    let affine = (ProjectivePoint::GENERATOR * scalar).to_affine();
    let mut out = [0u8; 33];
    out.copy_from_slice(affine.to_encoded_point(true).as_bytes());
    out
}

/// SEC1-uncompressed (65-byte `0x04 || X || Y`) public key for a private
/// scalar — Ethereum hashes `X || Y` with Keccak-256 for the address.
pub fn secp256k1_pubkey_uncompressed(priv_key: &[u8; 32]) -> Option<[u8; 65]> {
    let s = scalar_lt_n(priv_key).filter(|s| !bool::from(s.is_zero()))?;
    let affine = (ProjectivePoint::GENERATOR * s).to_affine();
    let mut out = [0u8; 65];
    out.copy_from_slice(affine.to_encoded_point(false).as_bytes());
    Some(out)
}

/// BIP-32 secp256k1 derivation from a master seed through `path`. Returns the
/// 32-byte leaf private key, or `None` on an invalid intermediate.
pub fn derive_bip32_secp256k1(seed: &[u8], path: &DerivationPath) -> Option<[u8; 32]> {
    derive_bip32_secp256k1_with_chaincode(seed, path).map(|(k, _)| k)
}

/// As [`derive_bip32_secp256k1`] but also returns the leaf 32-byte chain code
/// (Ethereum getAddress returns it when P2 requests it). Handles both hardened
/// (top-bit-set → `0x00 || ser256(k) || i`) and non-hardened
/// (`serP(point(k)) || i`) components, so standard BIP-44 wallet paths like
/// `m/44'/60'/0'/0/0` work. `None` on an invalid intermediate (`IL >= n` or a
/// zero child — both negligible).
pub fn derive_bip32_secp256k1_with_chaincode(
    seed: &[u8],
    path: &DerivationPath,
) -> Option<([u8; 32], [u8; 32])> {
    let i = hmac_sha512(SECP256K1_HMAC_KEY, seed);
    let mut key = scalar_lt_n(&i[0..32]).filter(|s| !bool::from(s.is_zero()))?;
    let mut chain_code = copy32(&i[32..64]);

    for idx in 0..path.depth as usize {
        let component = path.component(idx).unwrap_or(0);
        let hardened = component & 0x8000_0000 != 0;

        // hardened: 0x00 || ser256(k_par) || i ; non-hardened: serP(K_par) || i
        let mut data = [0u8; 33 + 4];
        if hardened {
            data[0] = 0x00;
            data[1..33].copy_from_slice(&key.to_bytes());
        } else {
            data[0..33].copy_from_slice(&compressed_pubkey(&key));
        }
        data[33..37].copy_from_slice(&component.to_be_bytes());

        let i = hmac_sha512(&chain_code, &data);
        let il = scalar_lt_n(&i[0..32])?;
        let child = il + key; // (IL + k_par) mod n
        if bool::from(child.is_zero()) {
            return None;
        }
        key = child;
        chain_code = copy32(&i[32..64]);
    }

    Some((key.to_bytes().into(), chain_code))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derivation_path::DerivationPath;
    use crate::state::SecretState;

    fn test_seed_state() -> SecretState {
        SecretState {
            secret_type: 0x02, // ImportedSeed
            secret_bytes: [
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
                0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
                0x1c, 0x1d, 0x1e, 0x1f,
            ],
        }
    }

    #[test]
    fn test_key_derivation_different_paths() {
        let state = test_seed_state();

        let path1_data = [
            0x02, // depth = 2
            0x80, 0x00, 0x00, 0x2c, // 44' (0x8000002c)
            0x80, 0x01, 0xf5, 0x00, // 501'/0' aka 0x8001f500 (legacy fixture)
        ];
        let path1 = DerivationPath::parse(&path1_data).unwrap();
        let keypair1 = derive_keypair(&state, path1);

        let path2_data = [
            0x02, // depth = 2
            0x80, 0x00, 0x00, 0x2c, // 44' (0x8000002c)
            0x80, 0x01, 0xf5, 0x01, // 501'/1' (0x8001f501)
        ];
        let path2 = DerivationPath::parse(&path2_data).unwrap();
        let keypair2 = derive_keypair(&state, path2);

        assert_ne!(keypair1.public.as_bytes(), keypair2.public.as_bytes());
    }

    #[test]
    fn test_key_derivation_same_path() {
        let state = test_seed_state();
        let path_data = [
            0x03, // depth = 3
            0x80, 0x00, 0x00, 0x2c, // 44' (0x8000002c)
            0x80, 0x01, 0xf5, 0x00, // 501'/0' (legacy fixture form)
            0x80, 0x00, 0x00, 0x00, // 0' (0x80000000)
        ];
        let path = DerivationPath::parse(&path_data).unwrap();

        let keypair1 = derive_keypair(&state, path);
        let path2 = DerivationPath::parse(&path_data).unwrap();
        let keypair2 = derive_keypair(&state, path2);

        assert_eq!(keypair1.public.as_bytes(), keypair2.public.as_bytes());
    }

    #[test]
    fn test_private_key_ignores_path() {
        // PrivateKey secret type bypasses derivation entirely — the 32
        // bytes are the Ed25519 seed verbatim. Same key for any path.
        let state = SecretState {
            secret_type: 0x01, // PrivateKey
            secret_bytes: [
                0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
                0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b,
                0x2c, 0x2d, 0x2e, 0x2f,
            ],
        };

        let path1_data = [0x02, 0x80, 0x00, 0x00, 0x2c, 0x80, 0x01, 0xf5, 0x00];
        let path1 = DerivationPath::parse(&path1_data).unwrap();
        let keypair1 = derive_keypair(&state, path1);

        let path2_data = [0x02, 0x80, 0x00, 0x00, 0x2c, 0x80, 0x01, 0xf5, 0x01];
        let path2 = DerivationPath::parse(&path2_data).unwrap();
        let keypair2 = derive_keypair(&state, path2);

        assert_eq!(keypair1.public.as_bytes(), keypair2.public.as_bytes());
    }

    /// SLIP-10 Ed25519 known answer: master seed = 0x000102…0f, path
    /// `m/0'`, expected private key per
    /// https://github.com/satoshilabs/slips/blob/master/slip-0010.md#test-vector-1-for-ed25519.
    #[test]
    fn test_slip10_ed25519_kat_master() {
        // Test vector 1 master from SLIP-10:
        //   master seed = 000102030405060708090a0b0c0d0e0f
        //   master priv = 2b4be7f19ee27bbf30c667b642d5f4aa69fd169872f8fc3059c08ebae2eb19e7
        let mut seed = [0u8; 32];
        seed[..16].copy_from_slice(&[
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ]);
        // Use a 16-byte seed here would actually deviate from our
        // 32-byte SecretState shape, so left-extend to 32 by zero-pad
        // (matches SLIP-10's tolerance of seeds 16..=64 B); but we want
        // to pin the canonical test vector, which is the 16-byte
        // master. Run the master-only HMAC directly:
        let i = hmac_sha512(ED25519_HMAC_KEY, &seed[..16]);
        let il = &i[0..32];
        let expected = [
            0x2b, 0x4b, 0xe7, 0xf1, 0x9e, 0xe2, 0x7b, 0xbf, 0x30, 0xc6, 0x67, 0xb6, 0x42, 0xd5,
            0xf4, 0xaa, 0x69, 0xfd, 0x16, 0x98, 0x72, 0xf8, 0xfc, 0x30, 0x59, 0xc0, 0x8e, 0xba,
            0xe2, 0xeb, 0x19, 0xe7,
        ];
        assert_eq!(il, expected);
    }

    /// BIP-32 secp256k1 known-answer from test vector 1
    /// (https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki):
    /// seed `000102…0f`, path `m/0'/1` (hardened then non-hardened) →
    /// private key `3c6cb8d0…93368`. Exercises both derivation modes.
    #[test]
    fn test_bip32_secp256k1_kat() {
        let seed = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        let path_data = [
            0x02, // depth = 2
            0x80, 0x00, 0x00, 0x00, // 0' (hardened)
            0x00, 0x00, 0x00, 0x01, // 1  (non-hardened)
        ];
        let path = DerivationPath::parse(&path_data).unwrap();
        let priv_key = derive_bip32_secp256k1(&seed, &path).unwrap();
        let expected = [
            0x3c, 0x6c, 0xb8, 0xd0, 0xf6, 0xa2, 0x64, 0xc9, 0x1e, 0xa8, 0xb5, 0x03, 0x0f, 0xad,
            0xaa, 0x8e, 0x53, 0x8b, 0x02, 0x0f, 0x0a, 0x38, 0x74, 0x21, 0xa1, 0x2d, 0xe9, 0x31,
            0x9d, 0xc9, 0x33, 0x68,
        ];
        assert_eq!(priv_key, expected);
    }
}
