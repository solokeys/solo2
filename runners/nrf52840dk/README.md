# solo2 on Nordic nRF52840-DK

### the first open source FIDO2 security key (on nRF, with NFC)

![nRF52840-DK with NFC antenna](https://www.crowdsupply.com/img/6714/d2036608-2b4c-4216-bd24-0e0cde406714/nrf52840-dk-with-nfc-antenna_png_md-fixed-xl.jpg)

> ⚠️ **Experimental — for developers only.**
> SoloKeys does not produce or sell any device using the Nordic nRF52840 chip.
> This runner is a research port of the solo2 firmware to the
> [nRF52840 Development Kit](https://www.nordicsemi.com/Products/Development-hardware/nRF52840-DK)
> for hacking, evaluation, and learning. **Do not** use it as a real
> security key — there is no provisioning, no attestation, no hardware
> root of trust, and no security review.

## What works

- USB CTAPHID — FIDO2 + U2F over USB (`fido-authenticator`)
- NFC — FIDO2 + U2F over NFC, plus an NDEF tag pointing at solokeys.com
  (a URL banner phones with a background NFC scanner can pop on tap)
- Storage — internal NVMC at `0x000C_0000` (256 KiB) for trussed-managed
  credentials; volatile RAM-backed for everything else

Tested with [webauthn.io](https://webauthn.io) (registration and
authentication) on the Nordic nRF52840-DK board.

## Build

```
make build-nrf
```

(or `cargo build --release` from this directory — cargo needs the
runner's `.cargo/config.toml` to pick up `target = thumbv7em-none-eabihf`,
so building with `-p` from the workspace root will fail.)

The output ELF lands at `target/thumbv7em-none-eabihf/release/runner-nrf52840dk`
(no `.elf` extension — cargo names the binary after the package).

## Flash

You need a J-Link probe (the DK has one onboard) and `JLinkExe`.

```
cd runners/nrf52840dk
cp ../../target/thumbv7em-none-eabihf/release/runner-nrf52840dk \
   ../../target/thumbv7em-none-eabihf/release/runner-nrf52840dk.elf
JLinkExe -device nRF52840_xxAA -if SWD -speed 4000 -autoconnect 1 \
   -CommandFile flash.jlink
```

The script halts the chip, loads the ELF, resets, and runs.

## Live RTT logs

```
JLinkRTTLogger -Device nRF52840_xxAA -If SWD -Speed 4000 -RTTChannel 0 out.log
defmt-print -e ../../target/thumbv7em-none-eabihf/release/runner-nrf52840dk.elf < out.log
```

`DEFMT_LOG=info` is set in `.cargo/config.toml`; raise to `debug` for
more chatter.

## Test

- USB: plug the J2 (USB) connector into a host. Open
  [webauthn.io](https://webauthn.io), pick "Register", confirm with
  SW1 or SW2 (any button → user-presence).
- NFC: tap the antenna pad on the DK with an NFC-capable phone. A phone
  running a background NFC scanner should pop a "https://solokeys.com/"
  banner. For WebAuthn over NFC, open webauthn.io in a browser that
  supports NFC security keys and choose "Security key" / "NFC" when
  prompted.

## NFC stack and the library

The NFC implementation depends on Nordic's **`libnfc_t4t.a`**
(Type 4 Tag library, distributed in [nRF Connect SDK]) — a
precompiled, **closed-source** binary blob that handles the ISO 14443-4
ISO-DEP layer (anti-collision, RATS/ATS, I-block framing, chaining, WTX).
The blob, the chip-layer `libnrfx_nfct.a`, and the platform glue are
all in [`components/nrf-nfc/`](../../components/nrf-nfc/); see that
crate's `vendor/README.md` for source/version provenance. The runner's
`nfct.rs` is a thin Rust wrapper that delivers reassembled APDUs to
`apdu-dispatch` and ships responses back through the library.

If you want a fully open NFC stack, the workspace also has an
open-source ISO-DEP layer in
[`components/nfc-device`](../../components/nfc-device). It isn't wired
into this runner today.

[nRF Connect SDK]: https://www.nordicsemi.com/Products/Development-software/nRF-Connect-SDK
