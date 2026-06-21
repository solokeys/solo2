# Password & secret management with a Solo key

> **Works on: Secure + Hacker.**

Two good ways to keep a password store unlockable only by your Solo — a modern age-based path and the classic GPG path.

## Modern: `passage` (age-backed, touch-to-unlock)
[`passage`](https://github.com/FiloSottile/passage) is a fork of `pass` that uses [age](AGE.md) instead of GPG. Pair it with `age-plugin-fido2-hmac` so every decrypt needs a Solo touch.
```bash
# 1. make an age identity bound to the Solo (see AGE.md)
age-plugin-fido2-hmac -g            # save the identity to ~/.passage/identities, recipient to ~/.passage/store/.age-recipients
# 2. use passage like pass
passage init                        # uses the recipient(s)
passage insert github/me
passage show github/me              # touch the Solo to decrypt
```

## Classic: `pass` (GPG on the OpenPGP applet)
Uses the Solo's OpenPGP card key; decrypting a password leaf happens on the card (PIN + touch depending on card config).
```bash
gpg --card-status                   # confirm your encryption key is on the Solo
pass init <GPG_KEY_ID>
pass insert github/me
pass show github/me                 # card decrypts the leaf
```
`gopass` works the same way. `git-crypt` (GPG) and `sops` (age or pgp) reuse these keys for repo-level secrets.

## With the `solo2` CLI
The `solo2` CLI (`cargo install solo2`) doesn't hold passwords itself — `pass`/`passage` run on the host and talk to the key via OpenPGP/FIDO2 directly. Use the CLI to **manage the device and its on-device secret store**:
```bash
solo2 ls                 # confirm the key + firmware
solo2 app oath list      # small secrets (TOTP/HOTP seeds) kept on the key — see OTP.md
solo2 app fido init      # confirm FIDO2 is live (for passkey-based vault logins below)
```

## Vaults
- **Bitwarden / 1Password:** add the Solo as a **passkey/2FA** (FIDO2) — protects the vault login, but the vault contents aren't on the key.
- **KeePassXC:** its classic hardware path is YubiKey HMAC-SHA1 challenge-response (different from Solo's OATH applet); a FIDO2/passkey unlock path may work — verify with your KeePassXC version.

See also: [AGE.md](AGE.md), [OTP.md](OTP.md).
