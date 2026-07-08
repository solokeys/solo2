---
name: solo2-cli
description: Use this when you need to install, run, or understand the `solo2` command-line tool — listing devices, updating firmware, or driving the on-device apps (admin, FIDO, OATH, PIV, NDEF). Use it whenever a task involves talking to a Solo 2 key from the host.
---

# Using the `solo2` CLI

`solo2` is the host tool for Solo 2 keys. Source is in [`cli/`](../../cli); run `solo2 --help` (or `cargo run -- --help`) for the authoritative, version-specific surface.

## Install / run

```bash
cargo install solo2            # installs the `solo2` binary
solo2 list                     # list connected devices (alias: solo2 ls)
cargo run -- list              # run from this repo without installing
```

Linux: install the udev rule [`cli/70-solo2.rules`](../../cli/70-solo2.rules); the CCID apps (OATH/PIV/OpenPGP) need a PC/SC stack (`pcscd`). macOS/Windows have PC/SC built in. FIDO is plain USB-HID (no driver).

## Top-level commands

- `solo2 list` — enumerate keys (shows UUID, firmware version, locked/unlocked).
- `solo2 update [--dry-run] [--yes] [--all] [--with <file.sb2>]` — install the latest **SoloKeys-signed** firmware; downloads, verifies SHA-256, flashes. Prompts on major updates. `--with` flashes a specific `.sb2`.
- `solo2 app <APP> …` — talk to an on-device application (below).
- `solo2 bootloader {list|reboot}` — interact with keys in bootloader mode.
- `solo2 pki ca fetch-certificate <R1|T1|S3|…>` — fetch Solo 2 PKI certs.
- `solo2 completion {bash|fish|zsh|powershell}` — shell completions.

`infer_subcommands` is on, so unambiguous prefixes work (e.g. `solo2 app fido i` → `init`).

## On-device apps (`solo2 app …`)

- **admin** — `wink` (blink LED to identify), `version`, `uuid`, `locked` (Secure=locked / Hacker=unlocked), `maintenance` (reboot into LPC55 bootloader), `restart` (reboot as Solo 2), `aid`.
- **fido** — `init`, `wink`. (Credential create/assert is done by a FIDO2 client/browser; see the `solo2-examples` skill.)
- **oath** — `register` (positional args or `--uri`), `list`, `totp`/`hotp`, `delete <label>`, `rename`, `reset`, `aid`. TOTP/HOTP secrets stored on the key.
- **piv** — PIV applet operations.
- **ndef** — `data`, `capabilities`, `aid` (tap-to-URL).

## Notes

- Identify which physical key you're talking to with `solo2 app admin wink` before destructive ops; target a specific key with the device selector flags (`solo2 --help`).
- `solo2 app admin locked` is the quick way to tell a Secure key (locked) from a Hacker (unlocked).
- For building/flashing firmware (not just talking to a key), see the `flash-solo-hacker` skill.
- Worked, tested command sequences live in [`examples/`](../../examples) — see the `solo2-examples` skill.
