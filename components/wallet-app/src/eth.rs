//! Ethereum address encoding for the Ledger getAddress response.

use sha3::{Digest, Keccak256};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// EIP-55 checksummed address — 40 ASCII hex chars (no `0x`) — from the 65-byte
/// uncompressed secp256k1 public key (`0x04 || X || Y`). The address is
/// `keccak256(X || Y)[12..32]`; the checksum upper-cases each `a–f` whose
/// matching nibble of `keccak256(lowercase-hex)` is ≥ 8.
pub fn address_ascii(pubkey_uncompressed: &[u8; 65]) -> [u8; 40] {
    let hash = Keccak256::digest(&pubkey_uncompressed[1..]);
    let addr = &hash[12..32];

    let mut out = [0u8; 40];
    for (i, b) in addr.iter().enumerate() {
        out[2 * i] = HEX[(b >> 4) as usize];
        out[2 * i + 1] = HEX[(b & 0x0f) as usize];
    }

    let check = Keccak256::digest(out);
    for i in 0..40 {
        let c = out[i];
        if c.is_ascii_alphabetic() {
            let nibble = (check[i / 2] >> (4 * (1 - (i % 2)))) & 0x0f;
            if nibble >= 8 {
                out[i] = c.to_ascii_uppercase();
            }
        }
    }
    out
}

/// `true` if `tx` starts with an EIP-2718 typed-tx envelope (type byte ≤ 0x7f),
/// e.g. EIP-1559 (`0x02`); `false` for a legacy RLP-list tx (`>= 0xc0`).
pub fn is_typed_tx(tx: &[u8]) -> bool {
    tx.first().is_some_and(|&b| b <= 0x7f)
}

/// Total encoded length of an RLP list whose header begins at `data[0]`
/// (`1 + payload` for a short list, `1 + len_of_len + payload` for a long one).
fn rlp_list_total(data: &[u8]) -> Option<usize> {
    let b = *data.first()?;
    if b < 0xc0 {
        return None; // not a list
    }
    if b <= 0xf7 {
        Some(1 + (b - 0xc0) as usize)
    } else {
        let lol = (b - 0xf7) as usize;
        let mut len = 0usize;
        for i in 0..lol {
            len = (len << 8) | *data.get(1 + i)? as usize;
        }
        Some(1 + lol + len)
    }
}

/// Total expected length of a (possibly typed) transaction, so the signer knows
/// when the chunked RLP is complete. Needs only the first few bytes.
pub fn tx_total_len(data: &[u8]) -> Option<usize> {
    if is_typed_tx(data) {
        Some(1 + rlp_list_total(data.get(1..)?)?)
    } else {
        rlp_list_total(data)
    }
}

/// Length of an RLP item's header at `data[off]` and its payload length, for a
/// single (non-nested) item: `(header_len, payload_len)`.
fn rlp_item(data: &[u8], off: usize) -> Option<(usize, usize)> {
    let b = *data.get(off)?;
    if b <= 0x7f {
        Some((0, 1)) // single byte, is its own value
    } else if b <= 0xb7 {
        Some((1, (b - 0x80) as usize))
    } else if b <= 0xbf {
        let lol = (b - 0xb7) as usize;
        let mut len = 0usize;
        for i in 0..lol {
            len = (len << 8) | *data.get(off + 1 + i)? as usize;
        }
        Some((1 + lol, len))
    } else {
        None // a nested list — not expected for the scalar fields we read
    }
}

/// EIP-191 `personal_sign` digest:
/// `keccak256("\x19Ethereum Signed Message:\n" + ascii(len) + message)`.
pub fn personal_message_hash(message: &[u8]) -> [u8; 32] {
    let mut h = Keccak256::new();
    h.update(b"\x19Ethereum Signed Message:\n");
    // Decimal ASCII of the message length, no_std.
    let mut buf = [0u8; 20];
    let mut n = message.len();
    let mut i = buf.len();
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    h.update(&buf[i..]);
    h.update(message);
    h.finalize().into()
}

/// chainId of a legacy EIP-155 transaction (the 7th RLP field:
/// `[nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0]`), as a u64.
pub fn legacy_chain_id(tx: &[u8]) -> Option<u64> {
    // Enter the list payload.
    let b = *tx.first()?;
    let list_hdr = if b <= 0xf7 {
        1
    } else {
        1 + (b - 0xf7) as usize
    };
    let mut off = list_hdr;
    // Skip fields 0..=5, read field 6.
    for _ in 0..6 {
        let (h, l) = rlp_item(tx, off)?;
        off += h + l;
    }
    let (h, l) = rlp_item(tx, off)?;
    let val = tx.get(off + h..off + h + l)?;
    let mut id = 0u64;
    for &byte in val {
        id = (id << 8) | byte as u64;
    }
    Some(id)
}
