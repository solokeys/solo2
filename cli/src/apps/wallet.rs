//! Wallet app over the Ledger-style HID transport: multi-chain key derivation
//! (Solana / Ethereum) and seed management.

use anyhow::anyhow;

use crate::Result;

wallet_app!();

/// Chain selected for `pubkey` — picks the default derivation path and the
/// address encoding.
#[derive(Clone, Copy)]
pub enum Chain {
    Sol,
    Eth,
}

impl Chain {
    fn default_path(self) -> &'static str {
        match self {
            // Matches Phantom / the Ledger Solana app (account 0, change 0).
            // `solana -k usb://ledger` (bare) uses the shorter m/44'/501';
            // pass `?key=0/0` there to get this same address.
            Chain::Sol => "m/44'/501'/0'/0'",
            Chain::Eth => "m/44'/60'/0'/0/0",
        }
    }
}

/// Serialize a BIP-32 path ("m/44'/60'/0'/0/0") as `[depth][u32 BE per
/// component]`, the high bit set for hardened components (so mixed
/// hardened/non-hardened paths — e.g. Ethereum — derive correctly).
fn serialize_path(path: &str) -> Result<Vec<u8>> {
    let path = path.strip_prefix("m/").unwrap_or(path);
    let mut components = Vec::new();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        let hardened = part.ends_with('\'') || part.ends_with('h') || part.ends_with('H');
        let digits = part.trim_end_matches(['\'', 'h', 'H']);
        let index: u32 = digits
            .parse()
            .map_err(|_| anyhow!("Invalid path component: {}", part))?;
        components.push(if hardened { index | 0x8000_0000 } else { index });
    }
    let mut out = vec![components.len() as u8];
    for c in components {
        out.extend_from_slice(&c.to_be_bytes());
    }
    Ok(out)
}

/// EIP-55 checksummed `0x…` address from a 20-byte address.
fn eip55(addr: &[u8]) -> String {
    use sha3::{Digest, Keccak256};
    let lower = hex::encode(addr);
    let hash = Keccak256::digest(lower.as_bytes());
    let mut out = String::from("0x");
    for (i, c) in lower.chars().enumerate() {
        if c.is_ascii_alphabetic() {
            let nibble = (hash[i / 2] >> (4 * (1 - (i % 2)))) & 0xf;
            out.push(if nibble >= 8 {
                c.to_ascii_uppercase()
            } else {
                c
            });
        } else {
            out.push(c);
        }
    }
    out
}

impl App {
    const CLA: u8 = 0xE0;
    const GET_PUBKEY: u8 = 0x05;
    const RESET: u8 = 0x10;
    const KEYGEN: u8 = 0x11;
    const SET_SEED: u8 = 0x12;
    const SET_PRIVATE_KEY: u8 = 0x13;
    const GET_SECRET_TYPE: u8 = 0x14;
    const SET_CHAIN: u8 = 0x16;

    /// Select the active chain the device presents to wallets (Ledger app
    /// detection). RAM only on the device — resets to Solana on unplug.
    pub fn set_chain(&mut self, chain: Chain) -> Result<()> {
        let p1 = match chain {
            Chain::Eth => 0x01,
            Chain::Sol => 0x00,
        };
        self.call_iso(Self::CLA, Self::SET_CHAIN, p1, 0x00, &[])?;
        Ok(())
    }

    /// Public key / address for a chain, using its default derivation path (or
    /// `path` if given), encoded as that chain expects.
    pub fn pubkey(&mut self, chain: Chain, path: Option<&str>) -> Result<String> {
        let path = path.unwrap_or_else(|| chain.default_path());
        let path_data = serialize_path(path)?;
        let response = self.call_iso(Self::CLA, Self::GET_PUBKEY, 0x00, 0x00, &path_data)?;

        match chain {
            // Solana: 32-byte Ed25519 pubkey -> base58.
            Chain::Sol => {
                if response.len() != 32 {
                    return Err(anyhow!("Invalid Solana pubkey length: {}", response.len()));
                }
                Ok(bs58::encode(&response).into_string())
            }
            // Ethereum: 65-byte uncompressed secp256k1 -> EIP-55 address of
            // keccak256(X||Y)[12..].
            Chain::Eth => {
                use sha3::{Digest, Keccak256};
                if response.len() != 65 {
                    return Err(anyhow!(
                        "Invalid Ethereum pubkey length: {}",
                        response.len()
                    ));
                }
                let hash = Keccak256::digest(&response[1..]);
                Ok(eip55(&hash[12..]))
            }
        }
    }

    /// Reset secret to zero private key.
    pub fn reset(&mut self) -> Result<()> {
        self.call_iso(Self::CLA, Self::RESET, 0x00, 0x00, &[])?;
        Ok(())
    }

    /// Generate a new seed. Returns the BIP39 words if `export`, else empty.
    pub fn keygen(&mut self, export: bool) -> Result<Vec<String>> {
        let p1 = if export { 0x01 } else { 0x00 };
        let response = self.call_iso(Self::CLA, Self::KEYGEN, p1, 0x00, &[])?;

        if export {
            if response.len() != 32 {
                return Err(anyhow!("Invalid seed length: {}", response.len()));
            }
            use bip39::Mnemonic;
            let mnemonic = Mnemonic::from_entropy(&response)
                .map_err(|e| anyhow!("Failed to create mnemonic: {}", e))?;
            Ok(mnemonic
                .to_string()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect())
        } else {
            Ok(vec![])
        }
    }

    /// Set seed from BIP39 words (ImportedSeed).
    pub fn seed(&mut self, words: Vec<String>) -> Result<()> {
        use bip39::Mnemonic;
        let phrase = words.join(" ");
        let mnemonic = Mnemonic::parse_in_normalized(bip39::Language::English, &phrase)
            .map_err(|e| anyhow!("Invalid mnemonic phrase: {}", e))?;
        let entropy = mnemonic.to_entropy();
        if entropy.len() < 32 {
            return Err(anyhow!("Invalid entropy length: {}", entropy.len()));
        }
        self.call_iso(Self::CLA, Self::SET_SEED, 0x00, 0x00, &entropy[..32])?;
        Ok(())
    }

    /// Read the secret type.
    pub fn seed_read(&mut self) -> Result<String> {
        let response = self.call_iso(Self::CLA, Self::GET_SECRET_TYPE, 0x00, 0x00, &[])?;
        if response.len() != 1 {
            return Err(anyhow!("Invalid response length: {}", response.len()));
        }
        Ok(match response[0] {
            0x00 => "empty",
            0x01 => "private_key",
            0x02 => "imported_seed",
            0x03 => "exported_seed",
            0x04 => "locked_seed",
            other => return Err(anyhow!("Unknown secret type: 0x{:02x}", other)),
        }
        .to_string())
    }

    /// Set private key from a file (JSON array of 64 bytes, solana keygen format).
    pub fn privkey(&mut self, file_path: &str) -> Result<()> {
        let contents = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow!("Failed to read file {}: {}", file_path, e))?;
        let key_bytes: Vec<u8> = serde_json::from_str(&contents)
            .map_err(|e| anyhow!("Failed to parse JSON array: {}", e))?;
        if key_bytes.len() != 64 {
            return Err(anyhow!(
                "Private key file must contain 64 bytes (32 private + 32 public), got {}",
                key_bytes.len()
            ));
        }
        let private_key: [u8; 32] = key_bytes[0..32]
            .try_into()
            .map_err(|_| anyhow!("Failed to extract private key"))?;
        let provided_public_key = &key_bytes[32..64];

        let keypair = salty::Keypair::from(&private_key);
        if keypair.public.as_bytes() != provided_public_key {
            return Err(anyhow!("Public key does not match private key"));
        }
        self.call_iso(Self::CLA, Self::SET_PRIVATE_KEY, 0x00, 0x00, &private_key)?;
        Ok(())
    }
}
