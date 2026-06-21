//! Wallet integration tests — multi-chain key derivation, signing, the secret-
//! management ops, and the user-presence gate.
//!
//! Each test drives `&mut dyn Wallet` (see `support/wallet_transport.rs`) and so
//! runs on either backend without change:
//!   - **sim** (default, CI): in-process `wallet_app::Authenticator::respond()`
//!     over the RAM Trussed runtime (`support/sim.rs`).
//!   - **device** (`WALLET_BACKEND=device`): the same APDUs framed over the
//!     Ledger-style USB HID transport to a real LPC55 / EVK. Drive UP over JTAG
//!     with `UP_BACKEND=probe-rs UP_CONTROL_ADDR=0x20043bfc PROBE_RS_CHIP=LPC55S69JBD100`
//!     and flash a buttons + `test-up-control` build.
//!
//! Determinism (RFC-6979 / Ed25519) lets us assert the on-curve verification AND
//! cross-check secp256k1 keys against the same KAT-verified derivation run
//! host-side, so the same test proves the device matches the reference.

use serial_test::serial;

#[path = "support/sim.rs"]
mod sim;
#[path = "support/up.rs"]
#[allow(dead_code)] // shared with the FIDO suite; the wallet uses a subset
mod up;
#[path = "support/wallet_transport.rs"]
mod wallet_transport;

use wallet_transport::{with_wallet, Wallet};

// ── fixtures ─────────────────────────────────────────────────────────────────

/// A 32-byte seed with a recognisable bit pattern for the deterministic tests.
const TEST_SEED: [u8; 32] = [0x42; 32];

/// Solana BIP-44 base path `m/44'/501'` (hardened, big-endian on the wire).
const SOLANA_PATH: [u8; 9] = [
    0x02, // depth
    0x80, 0x00, 0x00, 0x2c, // 44'
    0x80, 0x00, 0x00, 0xf5, // 501'
];

/// Ethereum BIP-44 path `m/44'/60'/0'/0/0` (coin_type 60 → secp256k1, Keccak).
const ETH_PATH: [u8; 21] = [
    0x05, // depth 5
    0x80, 0x00, 0x00, 0x2c, // 44'
    0x80, 0x00, 0x00, 0x3c, // 60'
    0x80, 0x00, 0x00, 0x00, // 0'
    0x00, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, // 0
];

const INS_ETH_GET_ADDRESS: u8 = 0x02;
const INS_ETH_SIGN_PERSONAL: u8 = 0x08;
const INS_GET_APP_CONFIGURATION: u8 = 0x04;
const INS_GET_PUBKEY: u8 = 0x05;
const INS_SIGN_MESSAGE: u8 = 0x06;
const INS_SIGN_OFFCHAIN_MESSAGE: u8 = 0x07;
/// Ledger P2 chunk flags for the Solana sign instructions.
const P2_EXTEND: u8 = 0x01;
const P2_MORE: u8 = 0x02;
const INS_RESET: u8 = 0x10;
const INS_KEYGEN: u8 = 0x11;
const INS_SET_SEED: u8 = 0x12;
const INS_SET_PRIVATE_KEY: u8 = 0x13;
const INS_GET_SECRET_TYPE: u8 = 0x14;
const INS_SET_CHAIN: u8 = 0x16;

const SW_DENIED: u16 = 0x6985; // ConditionsOfUseNotSatisfied
const SW_NOT_FOUND: u16 = 0x6a82; // FileOrAppNotFound — no secret set

// ── helpers ──────────────────────────────────────────────────────────────────

/// Send one APDU expecting success (`0x9000`); return the response body.
fn ok(w: &mut dyn Wallet, ins: u8, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
    w.transact(ins, p1, p2, data)
        .unwrap_or_else(|sw| panic!("APDU ins={ins:#04x} failed: sw={sw:#06x}"))
}

/// SIGN_MESSAGE payload: `num_paths(1) || path || message`.
fn sign_data(path: &[u8], message: &[u8]) -> Vec<u8> {
    let mut d = vec![1u8];
    d.extend_from_slice(path);
    d.extend_from_slice(message);
    d
}

/// Derive the secp256k1 uncompressed pubkey via wallet_app's KAT-verified
/// key_derivation directly (host-side), for cross-checking the device output.
fn expected_uncompressed(path: &[u8]) -> [u8; 65] {
    let state = wallet_app::SecretState {
        secret_type: 0x02, // ImportedSeed
        secret_bytes: TEST_SEED,
    };
    let dp = wallet_app::DerivationPath::parse(path).unwrap();
    let priv_key = wallet_app::key_derivation::derive_secp256k1_priv(&state, &dp).unwrap();
    wallet_app::key_derivation::secp256k1_pubkey_uncompressed(&priv_key).unwrap()
}

// ── seed → pubkey ────────────────────────────────────────────────────────────

#[test]
#[serial]
fn set_seed_then_get_pubkey_is_deterministic() {
    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);
        let pk1 = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH);
        // Re-seed with the same bytes and re-derive — must agree.
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);
        let pk2 = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH);

        assert_eq!(pk1.len(), 32, "Solana pubkey is 32 bytes");
        assert_eq!(pk1, pk2, "same seed must derive the same pubkey");
    });
}

#[test]
#[serial]
fn solana_sign_offchain_message_verifies() {
    // dApp sign-in (INS 0x07): the host sends a full Solana offchain message
    // (`\xffsolana offchain` domain prefix + header + content); the device
    // Ed25519-signs it verbatim, like signMessage.
    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);
        let pubkey: [u8; 32] = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH)
            .as_slice()
            .try_into()
            .unwrap();

        let mut message = Vec::new();
        message.push(0xff);
        message.extend_from_slice(b"solana offchain");
        message.extend_from_slice(b"sign in to dapp.example, nonce 42");

        let signature = ok(
            w,
            INS_SIGN_OFFCHAIN_MESSAGE,
            0,
            0,
            &sign_data(&SOLANA_PATH, &message),
        );
        assert_eq!(signature.len(), 64, "Ed25519 signatures are 64 bytes");

        let pk = salty::PublicKey::try_from(&pubkey).expect("valid Ed25519 pubkey");
        let sig_array: [u8; 64] = signature.as_slice().try_into().unwrap();
        assert!(
            pk.verify(&message, &salty::Signature::from(&sig_array))
                .is_ok(),
            "offchain signature must verify under the chip's reported pubkey",
        );
    });
}

/// Phantom chunks a sign-in across APDUs: first chunk P2_MORE carries
/// `[numPaths][path][msg part]`, the continuation P2_EXTEND carries the rest and
/// (MORE clear) triggers signing over the reassembled message.
#[test]
#[serial]
fn solana_sign_offchain_message_chunked() {
    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);
        let pubkey: [u8; 32] = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH)
            .as_slice()
            .try_into()
            .unwrap();

        let mut message = Vec::new();
        message.push(0xff);
        message.extend_from_slice(b"solana offchain");
        message.extend_from_slice(&[b'a'; 300]); // long enough to span two chunks
        let (part1, part2) = message.split_at(200);

        // First chunk (P2_MORE): path + start of message → empty 0x9000.
        let first = sign_data(&SOLANA_PATH, part1);
        assert_eq!(
            ok(w, INS_SIGN_OFFCHAIN_MESSAGE, 0x01, P2_MORE, &first),
            Vec::<u8>::new(),
            "intermediate chunk returns empty 0x9000",
        );
        // Continuation (P2_EXTEND, MORE clear): rest of message → signature.
        let signature = ok(w, INS_SIGN_OFFCHAIN_MESSAGE, 0x01, P2_EXTEND, part2);
        assert_eq!(
            signature.len(),
            64,
            "Ed25519 signature over reassembled msg"
        );

        let pk = salty::PublicKey::try_from(&pubkey).expect("valid Ed25519 pubkey");
        let sig_array: [u8; 64] = signature.as_slice().try_into().unwrap();
        assert!(
            pk.verify(&message, &salty::Signature::from(&sig_array))
                .is_ok(),
            "chunked signature must verify over the full message",
        );
    });
}

/// An off-chain message wrapped in the `\xffsolana offchain` envelope is signed
/// over its bare text, so a dApp verifying the SIWS message itself accepts it.
#[test]
#[serial]
fn solana_offchain_header_is_stripped_before_signing() {
    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);
        let pubkey: [u8; 32] = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH)
            .as_slice()
            .try_into()
            .unwrap();

        let text = b"example.com wants you to sign in\n\nNonce: abc123";
        // Envelope: domain || version(0) || format(1) || u16-LE len || text.
        let mut wrapped = Vec::new();
        wrapped.extend_from_slice(b"\xffsolana offchain");
        wrapped.push(0x00);
        wrapped.push(0x01);
        wrapped.extend_from_slice(&(text.len() as u16).to_le_bytes());
        wrapped.extend_from_slice(text);

        let signature = ok(
            w,
            INS_SIGN_OFFCHAIN_MESSAGE,
            0,
            0,
            &sign_data(&SOLANA_PATH, &wrapped),
        );

        let pk = salty::PublicKey::try_from(&pubkey).expect("valid Ed25519 pubkey");
        let sig_array: [u8; 64] = signature.as_slice().try_into().unwrap();
        // Verifies over the bare text, NOT the wrapped envelope.
        assert!(
            pk.verify(text, &salty::Signature::from(&sig_array)).is_ok(),
            "signature must verify over the unwrapped SIWS text",
        );
        assert!(
            pk.verify(&wrapped, &salty::Signature::from(&sig_array))
                .is_err(),
            "signature must NOT verify over the wrapped envelope",
        );
    });
}

#[test]
#[serial]
fn keygen_then_get_pubkey_returns_32_bytes() {
    with_wallet(|w| {
        ok(w, INS_KEYGEN, 0, 0, &[]); // P1=0 → locked, no export
        let pubkey = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH);
        assert_eq!(pubkey.len(), 32, "pubkey from a fresh keygen is 32 bytes");
        assert!(
            pubkey.iter().any(|&b| b != 0),
            "pubkey must not be all-zero (seed was written)",
        );
    });
}

// ── signing, per chain ───────────────────────────────────────────────────────

#[test]
#[serial]
fn solana_get_pubkey_and_sign() {
    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);
        let pubkey: [u8; 32] = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH)
            .as_slice()
            .try_into()
            .unwrap();

        let message = b"hello solana";
        let signature = ok(w, INS_SIGN_MESSAGE, 0, 0, &sign_data(&SOLANA_PATH, message));
        assert_eq!(signature.len(), 64, "Ed25519 signatures are 64 bytes");

        let pk = salty::PublicKey::try_from(&pubkey).expect("valid Ed25519 pubkey");
        let sig_array: [u8; 64] = signature.as_slice().try_into().unwrap();
        assert!(
            pk.verify(message, &salty::Signature::from(&sig_array))
                .is_ok(),
            "chip signature must verify under the chip's reported pubkey",
        );
    });
}

#[test]
#[serial]
fn ethereum_get_pubkey_and_sign() {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    use sha3::{Digest, Keccak256};

    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);

        let pubkey = ok(w, INS_GET_PUBKEY, 0, 0, &ETH_PATH);
        assert_eq!(pubkey.len(), 65, "eth pubkey is 65-byte uncompressed");
        assert_eq!(pubkey[0], 0x04, "uncompressed SEC1 prefix");
        assert_eq!(
            pubkey.as_slice(),
            &expected_uncompressed(&ETH_PATH)[..],
            "device pubkey must match the KAT-verified derivation",
        );

        let message = b"hello ethereum";
        let sig_bytes = ok(w, INS_SIGN_MESSAGE, 0, 0, &sign_data(&ETH_PATH, message));
        assert_eq!(sig_bytes.len(), 65, "eth signature is r||s||v (65 bytes)");

        // Recover the signer key and assert it equals the chip's pubkey —
        // proves the signature is valid AND the recovery id (v) is correct.
        let digest = Keccak256::digest(message);
        let sig = Signature::from_slice(&sig_bytes[..64]).unwrap();
        let recid = RecoveryId::from_byte(sig_bytes[64]).expect("valid recovery id");
        let recovered = VerifyingKey::recover_from_prehash(&digest, &sig, recid).unwrap();
        assert_eq!(
            recovered.to_encoded_point(false).as_bytes(),
            pubkey.as_slice(),
            "recovered pubkey must equal the chip's reported pubkey",
        );

        // RFC-6979 deterministic — signing again must yield identical bytes.
        let sig2 = ok(w, INS_SIGN_MESSAGE, 0, 0, &sign_data(&ETH_PATH, message));
        assert_eq!(sig_bytes, sig2, "ECDSA signing must be deterministic");
    });
}

/// Ledger Ethereum app protocol (what Phantom/MetaMask actually speak):
/// getAppConfiguration (INS 0x06, no data) + getAddress (INS 0x02).
#[test]
#[serial]
fn ethereum_ledger_get_address() {
    use sha3::{Digest, Keccak256};

    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);

        // getAppConfiguration (e0 06, no data) follows the explicit active chain
        // (default Solana). Solana → errors (host falls through to e0 04);
        // SET_CHAIN Ethereum → returns the eth config.
        assert_eq!(w.transact(INS_SIGN_MESSAGE, 0, 0, &[]), Err(0x6700));
        ok(w, INS_SET_CHAIN, 0x01, 0x00, &[]); // → Ethereum
        assert_eq!(
            ok(w, INS_SIGN_MESSAGE, 0, 0, &[]),
            vec![0x00, 0x01, 0x07, 0x02]
        );
        ok(w, INS_SET_CHAIN, 0x00, 0x00, &[]); // → Solana
        assert_eq!(w.transact(INS_SIGN_MESSAGE, 0, 0, &[]), Err(0x6700));

        // getAddress: e0 02, P2=01 (with chain code).
        // Response: [65][pubkey][40][ascii address][32 chaincode].
        let r = ok(w, INS_ETH_GET_ADDRESS, 0x00, 0x01, &ETH_PATH);
        assert_eq!(
            r.len(),
            1 + 65 + 1 + 40 + 32,
            "pubkey + address + chaincode"
        );
        assert_eq!(r[0], 65);
        let pubkey = &r[1..66];
        assert_eq!(
            pubkey,
            &expected_uncompressed(&ETH_PATH)[..],
            "getAddress pubkey"
        );
        assert_eq!(r[66], 40);
        let addr_ascii = core::str::from_utf8(&r[67..107]).unwrap();

        // The device's ASCII address must be the EIP-55 of keccak256(X||Y)[12..].
        let hash = Keccak256::digest(&pubkey[1..]);
        let raw = &hash[12..32];
        let lower = hex::encode(raw);
        let cksum = Keccak256::digest(lower.as_bytes());
        let expected: String = lower
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if c.is_ascii_alphabetic() && (cksum[i / 2] >> (4 * (1 - (i % 2))) & 0xf) >= 8 {
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            })
            .collect();
        assert_eq!(addr_ascii, expected, "EIP-55 checksummed address");

        // Without chain code (P2=0): 33 bytes shorter.
        let r2 = ok(w, INS_ETH_GET_ADDRESS, 0x00, 0x00, &ETH_PATH);
        assert_eq!(r2.len(), 1 + 65 + 1 + 40, "no chaincode when P2=0");
        assert_eq!(&r2[..107], &r[..107], "same pubkey + address");
    });
}

/// Recover the signer pubkey from a Ledger `[v|r|s]` tx signature over the tx's
/// keccak256, and assert it equals the account's key. `v_to_recid` extracts the
/// recovery id from the (typed or legacy) `v`.
fn assert_tx_sig(vrs: &[u8], tx: &[u8], v_to_recid: impl Fn(u8) -> u8) {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    use sha3::{Digest, Keccak256};
    assert_eq!(vrs.len(), 65, "[v|r|s] is 65 bytes");
    let digest = Keccak256::digest(tx);
    let sig = Signature::from_slice(&vrs[1..65]).unwrap();
    let recid = RecoveryId::from_byte(v_to_recid(vrs[0])).expect("valid recovery id");
    let recovered = VerifyingKey::recover_from_prehash(&digest, &sig, recid).unwrap();
    assert_eq!(
        recovered.to_encoded_point(false).as_bytes(),
        &expected_uncompressed(&ETH_PATH)[..],
        "recovered signer must equal the account key",
    );
}

#[test]
#[serial]
fn ethereum_sign_typed_transaction() {
    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);

        // Minimal EIP-1559-style typed tx: 0x02 || rlp([..]). Blind-signed.
        let tx = [0x02u8, 0xc3, 0x80, 0x01, 0x02];
        let mut data = ETH_PATH.to_vec(); // [pathLen][components]
        data.extend_from_slice(&tx);
        let vrs = ok(w, INS_GET_APP_CONFIGURATION, 0x00, 0x00, &data); // e0 04 P1=0

        // Typed tx → v is the recovery id (0/1).
        assert!(vrs[0] <= 1, "typed v is the recovery id");
        assert_tx_sig(&vrs, &tx, |v| v);
    });
}

#[test]
#[serial]
fn ethereum_sign_legacy_transaction_v() {
    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);

        // Legacy EIP-155 unsigned tx rlp([nonce, gasPrice, gasLimit, to, value,
        // data, chainId=1, 0, 0]) — minimal scalars.
        let tx = [
            0xc9u8, // list, 9 bytes payload
            0x80, 0x80, 0x80, 0x80, 0x80, 0x80, // nonce..data (all empty)
            0x01, // chainId = 1
            0x80, 0x80, // r, s = 0
        ];
        let mut data = ETH_PATH.to_vec();
        data.extend_from_slice(&tx);
        let vrs = ok(w, INS_GET_APP_CONFIGURATION, 0x00, 0x00, &data);

        // Legacy EIP-155: v = chainId*2 + 35 + recid → recid = v - 37 (chainId 1).
        assert!(vrs[0] == 37 || vrs[0] == 38, "v = 35 + 2*1 + recid");
        assert_tx_sig(&vrs, &tx, |v| v - 37);
    });
}

#[test]
#[serial]
fn ethereum_sign_personal_message() {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    use sha3::{Digest, Keccak256};

    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);

        let msg = b"Sign to derive your dApp key.";
        // e0 08 first chunk: [pathLen][path][msgLen(4B BE)][message].
        let mut data = ETH_PATH.to_vec();
        data.extend_from_slice(&(msg.len() as u32).to_be_bytes());
        data.extend_from_slice(msg);
        let vrs = ok(w, INS_ETH_SIGN_PERSONAL, 0x00, 0x00, &data);
        assert_eq!(vrs.len(), 65);
        assert!(vrs[0] == 27 || vrs[0] == 28, "personal_sign v = 27 + recid");

        // EIP-191 digest: keccak256("\x19Ethereum Signed Message:\n"+len+message).
        let mut h = Keccak256::new();
        h.update(b"\x19Ethereum Signed Message:\n");
        h.update(msg.len().to_string().as_bytes());
        h.update(msg);
        let digest = h.finalize();
        let sig = Signature::from_slice(&vrs[1..65]).unwrap();
        let recid = RecoveryId::from_byte(vrs[0] - 27).unwrap();
        let recovered = VerifyingKey::recover_from_prehash(&digest, &sig, recid).unwrap();
        assert_eq!(
            recovered.to_encoded_point(false).as_bytes(),
            &expected_uncompressed(&ETH_PATH)[..],
            "recovered signer must equal the account key",
        );
    });
}

/// A >255-byte sign payload forces an extended APDU (and, on device, multi-
/// packet HID). Signs a 2 KB message — the path a ~4 KB Solana tx will use.
#[test]
#[serial]
fn solana_sign_large_message_via_extended_apdu() {
    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);

        let message: Vec<u8> = (0..2000u32).map(|i| (i & 0xff) as u8).collect();
        let signature = ok(
            w,
            INS_SIGN_MESSAGE,
            0,
            0,
            &sign_data(&SOLANA_PATH, &message),
        );
        assert_eq!(
            signature.len(),
            64,
            "Ed25519 signature over the large message"
        );

        let pubkey: [u8; 32] = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH)
            .as_slice()
            .try_into()
            .unwrap();
        let pk = salty::PublicKey::try_from(&pubkey).unwrap();
        let sig_array: [u8; 64] = signature.as_slice().try_into().unwrap();
        assert!(
            pk.verify(&message, &salty::Signature::from(&sig_array))
                .is_ok(),
            "large-message signature must verify",
        );
    });
}

// ── user-presence gate ───────────────────────────────────────────────────────

/// Seed-affecting commands and signing require user presence: with consent
/// denied the command is rejected. `up::deny()`/`approve_sticky()` drive the
/// sim buttons (host) or the JTAG `UP_CONTROL` byte (device) identically.
/// keygen + set_seed stand in for the whole gated set (set_private_key, reset
/// share the one `require_user_presence` helper).
#[test]
#[serial]
fn seed_affecting_and_sign_commands_require_user_presence() {
    with_wallet(|w| {
        for (name, ins, data) in [
            ("keygen", INS_KEYGEN, &[][..]),
            ("set_seed", INS_SET_SEED, &TEST_SEED[..]),
        ] {
            up::deny();
            let result = w.transact(ins, 0, 0, data);
            up::approve_sticky();
            assert_eq!(result, Err(SW_DENIED), "{name} must require user presence");
        }

        // Store a seed (consent granted) so sign passes its precondition, then
        // deny: signing must still require presence.
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);
        up::deny();
        let result = w.transact(INS_SIGN_MESSAGE, 0, 0, &sign_data(&SOLANA_PATH, b"deny"));
        up::approve_sticky();
        assert_eq!(result, Err(SW_DENIED), "sign must require user presence");
    });
}

// ── secret-management ops (chain-agnostic — one test each) ───────────────────

/// `set_private_key` stores the 32 bytes as the key *verbatim*: derivation is
/// bypassed, so different paths under the same chain yield the same key.
#[test]
#[serial]
fn set_private_key_is_used_verbatim() {
    const KEY: [u8; 32] = [0x37; 32];
    // A deeper Solana path — derivation would change a seed-derived key, but a
    // raw private key must ignore the path entirely.
    const SOLANA_PATH_ALT: [u8; 13] = [
        0x03, // depth 3
        0x80, 0x00, 0x00, 0x2c, // 44'
        0x80, 0x00, 0x00, 0xf5, // 501'
        0x80, 0x00, 0x00, 0x07, // 7'
    ];
    with_wallet(|w| {
        ok(w, INS_SET_PRIVATE_KEY, 0, 0, &KEY);
        assert_eq!(
            ok(w, INS_GET_SECRET_TYPE, 0, 0, &[]),
            vec![0x01],
            "secret type is PrivateKey",
        );

        let pk = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH);
        let pk_alt = ok(w, INS_GET_PUBKEY, 0, 0, &SOLANA_PATH_ALT);
        assert_eq!(pk.len(), 32);
        assert_eq!(pk, pk_alt, "a raw private key ignores the derivation path");

        let message = b"raw key sign";
        let sig = ok(w, INS_SIGN_MESSAGE, 0, 0, &sign_data(&SOLANA_PATH, message));
        let pubkey: [u8; 32] = pk.as_slice().try_into().unwrap();
        let vk = salty::PublicKey::try_from(&pubkey).unwrap();
        let sig_array: [u8; 64] = sig.as_slice().try_into().unwrap();
        assert!(
            vk.verify(message, &salty::Signature::from(&sig_array))
                .is_ok(),
            "signature under the raw private key must verify",
        );
    });
}

/// `reset` wipes the secret: the type returns to Empty and signing fails.
#[test]
#[serial]
fn reset_wipes_the_secret() {
    with_wallet(|w| {
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);
        assert_eq!(
            ok(w, INS_GET_SECRET_TYPE, 0, 0, &[]),
            vec![0x02],
            "ImportedSeed before reset",
        );

        ok(w, INS_RESET, 0, 0, &[]);
        assert_eq!(
            ok(w, INS_GET_SECRET_TYPE, 0, 0, &[]),
            vec![0x00],
            "Empty after reset",
        );

        // With no secret, sign is rejected (the Empty check precedes any UP).
        let res = w.transact(INS_SIGN_MESSAGE, 0, 0, &sign_data(&SOLANA_PATH, b"x"));
        assert_eq!(
            res,
            Err(SW_NOT_FOUND),
            "sign fails once the secret is wiped"
        );
    });
}

/// `get_secret_type` tracks the active secret across every seed-management op.
#[test]
#[serial]
fn get_secret_type_tracks_the_secret() {
    with_wallet(|w| {
        let st = |w: &mut dyn Wallet| ok(w, INS_GET_SECRET_TYPE, 0, 0, &[])[0];

        // The device/sim store persists across tests, so normalise first.
        ok(w, INS_RESET, 0, 0, &[]);
        assert_eq!(st(w), 0x00, "Empty after reset");
        ok(w, INS_SET_SEED, 0, 0, &TEST_SEED);
        assert_eq!(st(w), 0x02, "ImportedSeed after set_seed");
        ok(w, INS_KEYGEN, 0, 0, &[]); // P1=0 → locked
        assert_eq!(st(w), 0x04, "LockedSeed after keygen");
        ok(w, INS_SET_PRIVATE_KEY, 0, 0, &[0x11; 32]);
        assert_eq!(st(w), 0x01, "PrivateKey after set_private_key");
        ok(w, INS_RESET, 0, 0, &[]);
        assert_eq!(st(w), 0x00, "Empty after reset");
    });
}

/// `get_app_configuration` reports the flags + firmware version.
#[test]
#[serial]
fn get_app_configuration_reports_version() {
    with_wallet(|w| {
        let cfg = ok(w, INS_GET_APP_CONFIGURATION, 0, 0, &[]);
        assert_eq!(cfg.len(), 5, "config payload is 5 bytes");
        assert_eq!(cfg[0], 0x01, "blind signing enabled");
        assert_eq!(&cfg[2..5], &[1, 7, 2], "firmware version 1.7.2");
    });
}
