---
name: flash-solo-hacker
description: Use this to build and flash custom firmware onto a Solo 2 Hacker key — including checking the lock/seal state (Secure vs Hacker, which is the PFR seal field, not secure boot), matching the build to the key's storage mode, and validating the equivalent build on an EVK first. Use it WHENEVER you are about to write firmware to a physical Hacker key, because a bad flash bricks it permanently.
---

# Build & flash a Solo 2 Hacker — safely

⚠️ **A Hacker key has no debug probe (no J-Link). A flash that doesn't boot + enumerate is permanently bricked — unrecoverable.** Read this whole skill before writing anything to a Hacker.

## Golden rules

1. **EVK-first.** If a dev board (LPC55S69-EVK) is available, build and validate the **equivalent EVK build** first (same source + features, just the EVK board target) — it has a J-Link and is fully recoverable. It is *not* a byte-identical binary — the EVK uses a different board target and no flash encryption — but it exercises the same code paths. Only flash the Hacker once the EVK build is proven. A bad shard/storage build has bricked a Hacker before.
2. **Match the build to the key's storage mode** (see step 2). Flashing a PRINCE-encrypted build onto a plain key (or vice-versa) bricks it.
3. **Never** attempt to flash custom firmware onto a **Secure** key — it only accepts SoloKeys-signed updates, and you must not try to unlock a user's production key (it won't work).

## 1. Build the firmware

```bash
git checkout <release-tag>              # reproduce a known release, or use your branch
make -C runners/lpc55 build-hacker      # → runners/lpc55/app-hacker.bin
# EVK equivalent for pre-validation:
make -C runners/lpc55 evk               # → app-hacker-evk.bin  (hacker feature set on the EVK)
```

## 2. Verify lock state and storage mode (Secure vs Hacker)

**Lock state.** "Secure vs Hacker" = **locked vs unlocked**, which is the PFR **`seal`** field — *not* secure boot. Read it two ways:

```bash
solo2 app admin locked     # quick: Hacker → unlocked, Secure → locked
lpc55 pfr native           # authoritative: the `seal` field — true = locked (Secure), false = unlocked (Hacker)
```

> ⚠️ **`seal` ≠ secure boot.** A key can have `secure_boot_enabled` set (and even the "Solo 2 Security Key" label) and still be **unlocked**. The `seal` field is the source of truth for locked/Secure vs unlocked/Hacker. **Do not flash a sealed (`seal = true`) key** — it's a Secure/production key.

**Storage mode.** Pick the matching build from the **KEYSTORE** (read the KEYSTORE, *not* the CMPA `prince_configuration`, which reads zero even when provisioned):

```bash
lpc55 pfr native           # inspect KEYSTORE: activation_code + prince_region_0/1/2
```

- KEYSTORE `activation_code` + `prince_region_*` **non-empty** → PRINCE-provisioned → flash a **`board-solo2`** (encrypted-storage) build.
- KEYSTORE **all-empty** → plain key → flash a **`no-encrypted-storage`** build.

This check is binary and exhaustive — do it before *every* Hacker flash.

## 3. Flash

The Hacker has **no J-Link** — flash via the LPC55 ROM bootloader with `lpc55 write-flash`:

```bash
solo2 app admin maintenance             # reboot Hacker into the LPC55 bootloader
solo2 bootloader list                   # confirm it appears in bootloader mode
lpc55 write-flash runners/lpc55/app-hacker.bin   # write the build matched in step 2
```

A **dev board** (EVK/DK) instead flashes over J-Link — fully recoverable, and where you should have validated first:

```bash
JLinkExe -device LPC55S69 -if SWD -speed 4000 -CommanderScript <(printf \
  'si SWD\nspeed 4000\ndevice LPC55S69\nconnect\nr\nh\nloadbin runners/lpc55/app-hacker-evk.bin 0x0\nr\ng\nexit\n')
```

## 4. Confirm it came back

After flashing, verify the key boots and enumerates:

```bash
solo2 list                              # should show the device + new version
solo2 app admin version
```

If a Hacker doesn't enumerate after a flash, try a button-hold replug to force the bootloader before assuming it's bricked. If it still won't enumerate, it's gone — which is exactly why **EVK-first** matters.
