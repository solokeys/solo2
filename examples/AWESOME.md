# Awesome Solo

A curated list of external tutorials and tools you can use with a Solo key. (For hands-on, tested walkthroughs in this repo see the other files in `examples/`.)

## FIDO2 / passkeys / WebAuthn
- WebAuthn demo & debugging — <https://webauthn.io>
- `ctap-hid-fido2` / `ctapcli` — generic Rust CTAP-HID CLI (PIN, bio, credMgmt, largeBlob) — <https://github.com/gebogebogebo/ctap-hid-fido2>
- python-fido2 (Yubico) — <https://github.com/Yubico/python-fido2>
- libfido2 (`fido2-token`/`-cred`/`-assert`) — <https://github.com/Yubico/libfido2>

## SSH (FIDO2 security-key keys)  → see [SSH.md](SSH.md)
- `ed25519-sk`/`ecdsa-sk` in the OpenSSH 8.2 release notes — <https://www.openssh.com/txt/release-8.2>

## Git commit signing  → see [GIT.md](GIT.md)
- GitHub: SSH commit signature verification — <https://docs.github.com/authentication/managing-commit-signature-verification/about-commit-signature-verification>

## Encryption: files & disk  → see [AGE.md](AGE.md)
- age — <https://age-encryption.org>
- `age-plugin-fido2-hmac` — <https://github.com/olastor/age-plugin-fido2-hmac>
- `typage` — encrypt to passkeys in the browser — <https://github.com/FiloSottile/typage>
- LUKS unlock via FIDO2 / TPM / PKCS#11 (systemd, 0pointer) — <https://0pointer.net/blog/unlocking-luks2-volumes-with-tpm2-fido2-pkcs11-security-hardware-on-systemd-248.html> ; ArchWiki — <https://wiki.archlinux.org/title/Systemd-cryptenroll>

## OATH (TOTP/HOTP)  → see [OTP.md](OTP.md)
- oath-toolkit (`oathtool`) — <https://www.nongnu.org/oath-toolkit/>

## Passwords & secrets  → see [PASS.md](PASS.md)
- `passage` (age-backed pass) — <https://github.com/FiloSottile/passage>
- `pass` (GPG) — <https://www.passwordstore.org>

## OpenPGP / PIV / smartcard
- `openpgp-card-tools` (`oct`, gpg-free) — <https://codeberg.org/openpgp-card/openpgp-card-tools>
- Container/blob signing with cosign + PKCS#11 — <https://docs.sigstore.dev/cosign/signing/pkcs11/>
- Run a CA with the key on the token: `step-ca` + PKCS#11 — <https://smallstep.com/docs/step-ca/cryptographic-protection/>
- DNSSEC zone signing with a token (Knot/BIND) — <https://en.blog.nic.cz/2016/06/13/dnssec-signing-with-knot-dns-and-yubikey/> ; SmartCard-HSM+BIND — <https://jpmens.net/2021/06/04/using-a-smartcard-hsm-for-dnssec-with-bind/>
- HashiCorp Vault PKCS#11 seal — <https://developer.hashicorp.com/vault/docs/configuration/seal/pkcs11>
- WireGuard private key on an OpenPGP card — <https://www.procustodibus.com/blog/2023/03/openpgpcard-wireguard-guide/>

## UEFI Secure Boot
- Sign your own Secure Boot keys (efikeygen/pesign) — <https://man.archlinux.org/man/extra/pesign/efikeygen.1.en>
- GRUB signature checking + a hardware key — <https://casualhacking.io/blog/2020/5/24/harden-your-linux-uefi-secure-boot-using-grub-signature-checking-and-a-yubikey>
- ArchWiki: UEFI Secure Boot — <https://wiki.archlinux.org/title/Unified_Extensible_Firmware_Interface/Secure_Boot>

## Post-quantum  → see [POST_QUANTUM.md](POST_QUANTUM.md), [TLOG.md](TLOG.md)
- OpenSSH post-quantum status — <https://www.openssh.com/pq.html>
- Open Quantum Safe (liboqs, OQS-OpenSSH) — <https://openquantumsafe.org>
- C2SP transparency-log specs (witness, cosignature with ML-DSA-44) — <https://c2sp.org/tlog-witness> ; <https://c2sp.org/tlog-cosignature>
- `filippo.io/mldsa` (proposed `crypto/mldsa`) / `filippo.io/torchwood` (tlog tooling) — <https://pkg.go.dev/filippo.io/mldsa>

## Mobile
- OpenKeychain (OpenPGP over USB/NFC, Android) — <https://www.openkeychain.org>
- Termux + OpenSSH `ed25519-sk` (libfido2) — <https://github.com/termux/termux-packages/issues/4942>

## Projects derived from Solo
- nixtropic — Nix-flake reproducible FIDO2/OpenPGP key on open silicon — <https://github.com/jjacke13/nixtropic>
