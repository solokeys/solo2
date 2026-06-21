# Solana App

A Trussed-based Solana wallet application for SoloKeys Solo 2 security keys. This app implements a Ledger-compatible Solana wallet that can generate and manage ed25519 keypairs for Solana blockchain transactions.

## Features

- **ed25519 Key Management**: Generate, import, and manage Solana keypairs
- **BIP Derivation Path Support**: Derive keys from seeds using hierarchical derivation paths
- **Multiple Secret Types**: Support for private keys, imported seeds, exported seeds, and locked seeds
- **User Presence**: Require physical button press for signing operations
- **Ledger HID Protocol**: Compatible with Ledger's HID transport protocol
- **Persistent Storage**: Secure storage of secrets using Trussed filesystem

## Architecture

The app consists of several modules:

- **`authenticator.rs`**: Core authenticator logic handling APDU commands
- **`command.rs`**: APDU command definitions and parsing
- **`derivation_path.rs`**: BIP derivation path parsing and handling
- **`key_derivation.rs`**: Key derivation logic for seeds and private keys
- **`state.rs`**: Persistent state management for secrets
- **`dispatch/`**: Custom HID dispatch layer for Ledger protocol
- **`usbd/`**: USB device implementation for HID communication

## APDU Commands

### GetAppConfiguration (0x04)
Returns the application configuration including version information.

**Request:**
- CLA: `0xE0` or `0x00`
- INS: `0x04`
- P1: `0x00`
- P2: `0x00`
- Data: Empty

**Response:**
- CBOR-encoded configuration vector with version string

### GetPubkey (0x05)
Get the public key for a given derivation path.

**Request:**
- CLA: `0xE0` or `0x00`
- INS: `0x05`
- P1: `0x00`
- P2: `0x00`
- Data: Serialized derivation path
  - Format: `[depth (1 byte)] + [depth * 4 bytes of components]`
  - Each component is a 4-byte big-endian u32 (hardened format: high bit set)

**Response:**
- 32-byte ed25519 public key

### SignMessage (0x06)
Sign a message using the key derived from the secret and derivation path.

**Request:**
- CLA: `0xE0` or `0x00`
- INS: `0x06`
- P1: `0x00`
- P2: `0x00`
- Data: `[num_paths (1 byte)] + [derivation path] + [message bytes]`

**Response:**
- 64-byte ed25519 signature

**Note:** Requires user presence (button press) with 30-second timeout.

### Reset (0x10)
Reset the secret to empty state (zero private key).

**Request:**
- CLA: `0xE0` or `0x00`
- INS: `0x10`
- P1: `0x00`
- P2: `0x00`
- Data: Empty

**Response:**
- Success status (0x9000)

### Keygen (0x11)
Generate a new random seed.

**Request:**
- CLA: `0xE0` or `0x00`
- INS: `0x11`
- P1: `0x00` (locked) or `0x01` (exported)
- P2: `0x00`
- Data: Empty

**Response:**
- If P1=0x01: 32-byte seed (for BIP39 conversion)
- If P1=0x00: Success status only

### SetSeed (0x12)
Set a seed from 32 bytes of entropy.

**Request:**
- CLA: `0xE0` or `0x00`
- INS: `0x12`
- P1: `0x00`
- P2: `0x00`
- Data: 32-byte seed

**Response:**
- Success status (0x9000)

### SetPrivateKey (0x13)
Set a private key directly (32 bytes).

**Request:**
- CLA: `0xE0` or `0x00`
- INS: `0x13`
- P1: `0x00`
- P2: `0x00`
- Data: 32-byte private key

**Response:**
- Success status (0x9000)

### GetSecretType (0x14)
Get the current secret type.

**Request:**
- CLA: `0xE0` or `0x00`
- INS: `0x14`
- P1: `0x00`
- P2: `0x00`
- Data: Empty

**Response:**
- 1 byte: Secret type (0x00=Empty, 0x01=PrivateKey, 0x02=ImportedSeed, 0x03=ExportedSeed, 0x04=LockedSeed)

## Secret Types

The app supports different types of secrets:

- **Empty (0x00)**: No secret set. `get_pubkey` and `sign_message` will fail.
- **PrivateKey (0x01)**: Direct private key. Used as-is, derivation path is ignored.
- **ImportedSeed (0x02)**: Seed imported from CLI. Derives keys using the derivation path.
- **ExportedSeed (0x03)**: Seed generated with `keygen` and exported (BIP39 words shown). Derives keys using the derivation path.
- **LockedSeed (0x04)**: Seed generated with `keygen` in silent mode (BIP39 words not shown). Derives keys using the derivation path.

## Key Derivation

- **PrivateKey**: Uses the secret bytes directly as the ed25519 keypair, ignoring the derivation path.
- **Seed types**: Performs key derivation by mixing the seed with each path component. The derivation uses XOR, addition, rotation, and byte swapping operations.

**Note:** The current derivation is a simple mixing function suitable for testing. For production, this should be replaced with proper SLIP-10 or BIP32 derivation.

## User Presence

The `SignMessage` command requires user presence (physical button press) with a 30-second timeout. If the user doesn't press the button within 30 seconds, the command returns `Status::ConditionsOfUseNotSatisfied` (0x6985).

The state is **not** reset on timeout - the secret remains unchanged and other commands (like `get_pubkey`) continue to work normally.

## Building

The app is built as part of the Solo 2 runner. To build with the Solana app enabled:

```bash
cd solo2/runners/lpc55
make build-dev
```

Or with specific features:

```bash
cargo build --release --features board-lpcxpresso55,develop,log-all,solana-app/log-all
```

## Testing

Run tests for the solana-app crate:

```bash
cd solana
cargo test
cargo fmt
```

## CLI Usage

The app can be used via the `solo2-cli` tool:

```bash
# Get public key
solo2 app solana pubkey

# Get public key for specific path
solo2 app solana pubkey --path "m/44'/501'/0'/0'"

# Generate a new seed (with BIP39 words)
solo2 app solana keygen

# Generate a new seed (silent, no words shown)
solo2 app solana keygen --silent

# Set seed from BIP39 words
solo2 app solana seed word1 word2 ... word24

# Read secret type
solo2 app solana seed --read

# Set private key from file (JSON array of 64 bytes)
solo2 app solana privkey /path/to/key.json

# Reset secret
solo2 app solana reset
```

## Protocol

The app uses a custom HID dispatch layer (`solana-app/dispatch`) that implements the Ledger HID transport protocol. This allows direct communication with the device via USB HID, bypassing the standard APDU dispatch used by other apps.

The transport protocol wraps ISO7816 APDU commands in a 5-byte header:
- Byte 0-1: Channel ID (0x01, 0x01)
- Byte 2: Tag (0x05 for APDU)
- Byte 3-4: Sequence number

## License

Apache-2.0 OR MIT
