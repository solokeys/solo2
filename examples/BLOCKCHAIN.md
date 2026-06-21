# Blockchain wallet with a Solo key

> **Works on: Hacker** (the multi-chain `wallet` build; USB-identity spoofing is hacker-only).

Solo 2 can act as a **multi-chain hardware wallet** — Ed25519 for **Solana** and secp256k1 for **Ethereum / EVM** — over a Ledger-style USB HID transport. Keys are derived on-device from a BIP-39 seed (SLIP-10 for Solana, BIP-32 for Ethereum); every signature needs a **touch** (user presence). Optionally the device can present itself as a **Ledger** so existing tools (the `solana` CLI, Phantom, MetaMask) talk to it unmodified.

## 1. Keys

```bash
cargo install solo2                 # provides the `solo2` binary
solo2 ls                            # devices + firmware, e.g. "Solo 2 … (CTAP+PCSC, firmware 2:…)"

# Generate a fresh seed (writes down 24 BIP-39 words — back them up!)
solo2 app wallet keygen             # add --silent to keep the words off-screen

# …or import an existing 24-word seed, and check what's stored
solo2 app wallet seed word1 word2 … word24
solo2 app wallet seed --read        # -> imported_seed | exported_seed | private_key | empty
```

Read the public address for each chain (default derivation paths shown):

```bash
solo2 app wallet pubkey --sol       # Solana, base58   (m/44'/501'/0'/0' — matches Phantom)
solo2 app wallet pubkey --eth       # Ethereum, EIP-55 (0x…, m/44'/60'/0'/0/0)

# Override the path for a different account/index:
solo2 app wallet pubkey --sol --path "m/44'/501'/1'/0'"
```

`--sol` is the default, so a bare `solo2 app wallet pubkey` prints the Solana address.

### Optional — emulate a Ledger

So the `solana` CLI / Phantom / MetaMask recognise the device, give it Ledger's USB identity (`0x2c97:0x7000`) and **replug**:

```bash
solo2 app admin set usb --vid 2c97 --pid 7000 --manufacturer Ledger --product "Nano S Plus"
# unplug and replug — USB params apply at enumeration

# revert to the SoloKeys identity (0x1209:0xbeee) when done:
solo2 app admin set usb --default
```

## 2. Solana, with the `solana` CLI

With the device emulating a Ledger (step 1), the standard [Solana CLI](https://docs.solanalabs.com/cli/install) drives it directly. Use `?key=0/0` to match `pubkey --sol` (a bare `usb://ledger` uses the shorter `m/44'/501'`).

```bash
solana config set --url devnet

# Address (should equal `solo2 app wallet pubkey --sol`)
solana address --keypair "usb://ledger?key=0/0"

# Fund it on devnet, then check the balance
solana airdrop 1 "usb://ledger?key=0/0"
solana balance "usb://ledger?key=0/0"

# Send a transfer — touch the Solo to approve (the LED breathes blue while it waits)
solana transfer --keypair "usb://ledger?key=0/0" <RECIPIENT_ADDRESS> 0.1 \
    --allow-unfunded-recipient
```

Every signing operation blocks on **user presence** — tap the device when it asks.

## 3. Browser wallets — Phantom & MetaMask

The device speaks the Ledger app protocol, so connect it as a **hardware wallet**:

- **Phantom (Solana):** add a hardware wallet → Ledger. Solana is the device's default chain, so it connects, signs transactions, and signs in (SIWS) out of the box.
- **Phantom (EVM) / MetaMask:** these expect the *Ethereum* Ledger app. Tell the device to present Ethereum first, then connect:

```bash
solo2 app wallet set-chain --eth     # device now answers the Ethereum app probe
# …connect / sign in MetaMask or Phantom-EVM…
solo2 app wallet set-chain --sol     # back to Solana (or just replug — it resets to Solana)
```

The active chain is **explicit RAM state** (it resets to Solana on every replug): a wallet's app-detection probe is byte-identical for Solana and Ethereum, so the device can't guess — you pick the chain like you'd open an app on a real Ledger. MetaMask works on either setting, since it asks for the address directly rather than probing.
