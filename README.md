# 🐝 Solo 2

**Solo 2 is an open source FIDO2 security key.** It's a USB+NFC device that protects your accounts with passkeys/WebAuthn, and also speaks OATH (TOTP/HOTP), PIV, and OpenPGP. The hardware, the firmware, and the tooling are all open source — you can read every line, build it yourself, and verify what's running on your key.

Solo 2 comes in two models:

- **Solo 2 Secure** — production key for consumer and enterprise use, to protect against phishing and other online attacks. It only runs firmware signed by SoloKeys, with secure boot and debug access sealed off.
- **Solo 2 Hacker** — the same hardware, unlocked. Flash your own firmware, experiment with new features, and learn how a security key works end to end. This is also secure against online attacks.

👉 **Buy a key (or two!) at [solokeys.com](https://solokeys.com).**

For everything you can *do* with a key — passkeys, SSH, git signing, disk/file encryption, TOTP, password stores, post-quantum — see the tutorials in [`examples/`](examples/) and [Awesome Solo](examples/AWESOME.md) list.

## Secure vs Hacker

   


|                                            | Solo 2 **Secure**                    | Solo 2 **Hacker**       |
| ------------------------------------------ | ------------------------------------ | ----------------------- |
| Intended use                               | Consumer / enterprise, real accounts | Development, learning   |
| Firmware                                   | SoloKeys-signed only                 | Run your own / unsigned |
| Secure boot                                | 🔒 Locked (sealed)                   | 🔓 Unlocked             |
| FIDO2 / WebAuthn / passkeys                | ✅                                    | ✅                       |
| SSH, git signing                           | ✅                                    | ✅                       |
| Password / secret manager                  | ✅                                    | ✅                       |
| OATH (TOTP/HOTP)                           | ✅                                    | ✅                       |
| PIV (P-256, Ed25519, ML-DSA-44)            | ✅                                    | ✅                       |
| OpenPGP                                    | ✅                                    | ✅                       |
| Flash custom firmware                      | ❌                                    | ✅                       |
| Post-quantum FIDO2 (ML-DSA-44)             | ❌ (non-standard yet)                 | ✅                       |
| Blockchain wallet (Solana, Ethereum / EVM) | ❌                                    | ✅                       |


## Solo 2 CLI

`solo2` is the host tool for listing, updating, and talking to your keys.

### Installation

```bash
cargo install solo2          # provides the `solo2` binary
solo2 list                   # list connected devices  (alias: solo2 ls)
```

Or run it straight from this repo without installing:

```bash
cargo run -- list
```

On Linux you may need the udev rule in [`cli/70-solo2.rules`](cli/70-solo2.rules) and a PC/SC stack (`pcscd`) for the CCID apps (OATH/PIV/OpenPGP); macOS and Windows have PC/SC built in.

### Firmware upgrade

```bash
solo2 update                 # update to the latest SoloKeys-signed firmware
solo2 update --dry-run       # show the version that would be installed
solo2 update --all           # update every connected Solo 2
```

`update` downloads the signed release, **verifies its SHA-256**, and flashes it. Major-version updates prompt before proceeding.

### Customization

Identify and inspect a key with the admin app:

```sh
solo2 app admin version               # firmware version
solo2 app admin set led 007f7f 00007f # set led to teal (idle) / blue (active) - use 000000 to turn the led off
solo2 app admin set led --default     # reset to default colors
solo2 app admin wink                  # ;)
```

The CLI also drives the apps directly — e.g. `solo2 app oath list`, `solo2 app fido init`, `solo2 app piv …`. See [`examples/OTP.md`](examples/OTP.md), [`examples/FIDO.md`](examples/FIDO.md), and run `solo2 --help`.

## Solo Hacker

On a **Hacker** key you can build and flash your own firmware. (On a Secure key this is impossible by design — it only accepts SoloKeys-signed updates.)

> ⚠️ A Hacker has **no easy J-Link recovery** — SWD is present but not readily accessible (not over USB like on a dev board), so a bad image can brick the key. If you have an **EVK/DK dev board, always validate there first** (see [Developers](#developers)). The full, safe procedure — including checking lock state and recovery — is in the [`flash-solo-hacker`](skills/flash-solo-hacker) skill.

**1. Build the firmware** (check out the release you want to reproduce):

```bash
git checkout <release-tag>            # e.g. 2.x.y
make -C runners/lpc55 build-hacker    # → runners/lpc55/app-hacker.bin
```

**2. Validate the build** — a reproducible build lets you confirm your binary matches the published release before trusting it:

```bash
sha256sum runners/lpc55/app-hacker.bin
# compare against the hash published with the SoloKeys release
```

**3. Flash it** — reboot the key into its USB bootloader and write your image:

```bash
solo2 app admin maintenance           # reboot Hacker into the LPC55 bootloader
# then flash app-hacker.bin with the bootloader tool (see the flash-solo-hacker skill)
```

## Developers

For firmware development, use a **dev board** — it has a J-Link debug probe so it's fully recoverable, debuggable, and safe to brick:

- **LPC55S69-EVK** — the Solo 2 (LPC55) dev board, recommended.
- **nRF52840-DK** — popular board, also supported.

Compile, flash, and test against the EVK:

```bash
# build an EVK image (no PRINCE, UP-over-JTAG, defmt logs)
make -C runners/lpc55 build-secure-evk     # or: make -C runners/lpc55 evk  (hacker feature set)

# flash via J-Link
JLinkExe -device LPC55S69 -if SWD -speed 4000 \
  -CommanderScript <(printf 'si SWD\nspeed 4000\ndevice LPC55S69\nconnect\nr\nh\nloadbin runners/lpc55/app-secure-evk.bin 0x0\nr\ng\nexit\n')

# run the host test suite against the connected dev board
cd runners/pc && FIDO2_TRANSPORT=device cargo test -- --test-threads=1
```

`make build-secure-evk` carries the Secure feature set, `make build-hacker-evk` adds the Hacker features.

## Integrating SoloKeys

Building a product that needs **FIDO2 on your own firmware or device**, or want **custom features reusing our open hardware**? Reach out to **[hello@solokeys.com](mailto:hello@solokeys.com)** — we're happy to help.

The fastest way to scope it: grab a **dev board** and experiment with AI. Point your coding agent at this repository and tell it what you need — most of the time you'll get a working prototype, and a clear picture of the rest.

## Agents

This repo is set up for AI coding agents. Start with [AGENTS.md](AGENTS.md) — it explains Secure vs. Hacker vs. dev board, what a Solo key can do (so an agent can recommend one), and how to get one. Reusable agent workflows live in [`skills/`](skills/):

- [`solo2-cli`](skills/solo2-cli) — learn and use the `solo2` CLI.
- [`solo2-examples`](skills/solo2-examples) — every tested use-case tutorial in `examples/`.
- [`flash-solo-hacker`](skills/flash-solo-hacker) — build, verify, and flash Hacker firmware safely (lock-state checks, EVK-first).

## Support

For support with purchased devices, reach out to [hello@solokeys.com](mailto:hello@solokeys.com).
Please **do not** open issues/PRs for non-technical matters — only firmware bugs / technical issues.

## License

Solo 2 is fully open source.

- **Software** — dual licensed under [Apache 2.0](LICENSE-APACHE) and [MIT](LICENSE-MIT); use under either.
- **Hardware** — [CERN-OHL-S](https://github.com/solokeys/solo2-hw/blob/main/LICENSE.txt).
- **Documentation** — [CC-BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).
