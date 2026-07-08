---
name: solo2-examples
description: Use this to find the right tested, hands-on tutorial for doing something real with a Solo 2 key — passkeys, SSH, git signing, file/disk encryption, TOTP/HOTP, password stores, or post-quantum signing. Use it when a user asks "how do I use my Solo for X" or when recommending a concrete workflow.
---

# Solo 2 examples / use-case tutorials

The [`examples/`](../../examples) directory holds **tested, end-to-end** walkthroughs. Each file has a `> Works on: …` banner stating Secure and/or Hacker. Point users to the matching file; for a broader (untested) link list see [`examples/AWESOME.md`](../../examples/AWESOME.md).

## Index

| File | Use case | Works on | Key tools |
|---|---|---|---|
| [FIDO.md](../../examples/FIDO.md) | FIDO2/WebAuthn passkeys, `hmac-secret`, the COSE algorithms | Secure + Hacker | browser, python-fido2, `solo2`, libfido2 |
| [SSH.md](../../examples/SSH.md) | SSH keys bound to the token (`ed25519-sk`/`ecdsa-sk`) | Secure + Hacker | OpenSSH ≥ 8.2 |
| [GIT.md](../../examples/GIT.md) | Signing git commits (and optional push auth) with the key | Secure + Hacker | git, SSH-signing |
| [AGE.md](../../examples/AGE.md) | File/disk encryption, touch-to-decrypt | Secure + Hacker | `age` + `age-plugin-fido2-hmac` |
| [OTP.md](../../examples/OTP.md) | TOTP/HOTP 2FA codes generated on the key | Secure + Hacker | `solo2 app oath`, oath-toolkit |
| [PASS.md](../../examples/PASS.md) | Password stores; passkey/2FA for vaults | Secure + Hacker | `pass`/`passage`, Bitwarden/1Password |
| [POST_QUANTUM.md](../../examples/POST_QUANTUM.md) | ML-DSA-44 signing — **PIV (Secure+Hacker)**, **FIDO2 `-50` (Hacker only)** | mixed (see banner) | python-fido2, pyscard, OpenSSL ≥ 3.5, liboqs |
| [TLOG.md](../../examples/TLOG.md) | Solo as a post-quantum transparency-log witness (ML-DSA cosignature) | Secure + Hacker | python, `filippo.io/mldsa` (Go) |
| [AWESOME.md](../../examples/AWESOME.md) | Curated external links by category (PIV/OpenPGP, LUKS, DNSSEC, CA-on-token, …) | — | — |

Runnable demo scripts for the post-quantum tutorials are under [`examples/post_quantum/`](../../examples/post_quantum) (`pq_fido2_demo.py`, `pq_piv_demo.py`, `pq_tlog_witness.py`, and the Go verifier `tlog_verify/`).

## How to use

1. Identify the user's goal and pick the row above.
2. **Check the `Works on:` banner** — if they have a Secure key, don't suggest a Hacker-only path (e.g. FIDO2 ML-DSA `-50`). PIV ML-DSA works on Secure.
3. Follow the file's tested commands. Transports: FIDO2 is USB-HID (driverless); OATH/PIV/OpenPGP are USB-CCID (need PC/SC — built into macOS/Windows, `pcscd` on Linux); NFC only on B-variant keys.
4. If a user doesn't have a key yet, they can buy one at [solokeys.com](https://solokeys.com) (Secure for real use, Hacker for tinkering).
