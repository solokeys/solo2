# ─────────────────────────────────────────────────────────────────────────────
# Solo 2 lpc55 firmware build targets
#
# `make build-<type>` builds + objcopies a `app-<type>.bin` for each key model.
# Feature axes: board-solo2 vs board-lpcxpresso55 (EVK); PRINCE encrypted storage
# (ON = board-solo2 alone, OFF = add no-encrypted-storage, mandatory on EVK);
# buttons (no-buttons auto-succeeds UP; test-up-control drives UP over JTAG on EVK);
# the secure baseline (the crate's DEFAULT features, incl. PIV ML-DSA-44, always on);
# the `hacker` Cargo feature, capabilities hacker adds on top (currently FIDO ML-DSA-44);
# provisioner-app; log-defmt.
#
#   PRODUCTS — real hardware, PRINCE-encrypted storage, buttons:
#     secure       board-solo2, baseline (incl. PIV ML-DSA)  → app-secure.bin (sign externally)
#     hacker       board-solo2, baseline + new features       → app-hacker.bin
#
#   EVK — LPCXpresso55S69-EVK: no PRINCE, buttons + UP-over-JTAG, defmt logs:
#     secure-evk   secure feature set on EVK        → app-secure-evk.bin
#     hacker-evk   hacker feature set on EVK        → app-hacker-evk.bin
#     evk          alias for hacker-evk
#
#   DEV — real solo2, no buttons so it can be auto-rebooted into the bootloader and
#         reflashed. No log-defmt (a real key has no JTAG/RTT to read it).
#     dev / devel  hacker feature set, no buttons   → app-dev.bin
#
#   Described in the flash-solo-boards skill but intentionally NOT targets here — internal /
#   ad hoc, build by hand (ask if you need one):
#     secure-dev   secure feature set + no-buttons.
#     preprod      secure feature set + provisioner-app, to simulate a secure (add attestation);
#                  build one when you need to reflash a secure-like key. Internal only.
#                  (both inherit PIV ML-DSA from the secure baseline.)
#
# PRINCE: ON for secure/hacker and for DEV (it runs on an existing hacker, which is
# PRINCE-provisioned). OFF only on the EVK (no PRINCE hardware → no-encrypted-storage / RAM
# stand-in). dev stays reflashable via no-buttons, not by dropping PRINCE.
#
# Recommended flow: test changes on EVK, then on DEV, then build SECURE to sign.
# Custom set? Just compose: e.g. recovery = append `,format-filesystem`.
# ─────────────────────────────────────────────────────────────────────────────

BOARD := lpcxpresso55

# $(call build_fw,<type>,<feature-list-var>) — cargo build + objcopy to app-<type>.bin.
# NOTE: $(2) MUST be a single variable holding the comma-separated features —
# do NOT inline commas here, $(call) treats literal commas as arg separators.
define build_fw
	cargo build   --release --features $(2)
	cargo objcopy --release --features $(2) -- -O binary app-$(1).bin
endef

# The secure baseline (apps + PIV ML-DSA) is the crate's DEFAULT feature set, so it's
# implied by every build. Hacker-only extras live in the `hacker` Cargo feature.
# Per-type feature lists (commas live inside the variable, safe for $(call)).
SECURE     := board-solo2
HACKER     := board-solo2,hacker
# Shared EVK base: no PRINCE (RAM stand-in), UP over JTAG, defmt logging, lax reset window.
EVK_BASE   := board-lpcxpresso55,no-encrypted-storage,test-up-control,log-defmt,no-reset-time-window
SECURE_EVK := $(EVK_BASE)
HACKER_EVK := $(EVK_BASE),hacker
# dev = hacker (PRINCE on — runs on a real hacker) + no-buttons so the host can force the
# ROM bootloader and reflash unattended. No log-defmt (a real key has no JTAG/RTT).
DEV        := board-solo2,no-buttons,hacker

.PHONY: build-secure build-hacker build-secure-evk build-hacker-evk evk \
        build-dev build-devel

# ── Products (PRINCE, buttons, real solo2) ───────────────────────────────────
build-secure:
	$(call build_fw,secure,$(SECURE))

build-hacker:
	$(call build_fw,hacker,$(HACKER))

# ── EVK (no PRINCE, buttons + UP-over-JTAG, logs) ────────────────────────────
build-secure-evk:
	$(call build_fw,secure-evk,$(SECURE_EVK))

build-hacker-evk:
	$(call build_fw,hacker-evk,$(HACKER_EVK))

# `evk` alone == hacker-evk
evk: build-hacker-evk

# ── Dev (real solo2, PRINCE on, no buttons → auto-reflashable) ────────────────
build-dev:
	$(call build_fw,dev,$(DEV))

# alias
build-devel: build-dev

# ─────────────────────────────────────────────────────────────────────────────
.PHONY: today-in-days bacon jlink get-cmsis-pack
today-in-days:
	python -c "from datetime import date as d; print((d.today() - d(2020, 1, 1)).days)"

bacon:
	bacon

jlink:
	../../scripts/bump-jlink
	JLinkGDBServer -strict -device LPC55S69 -if SWD -vd

PACK_VERSION := $(shell wget -O - -qq https://mcuxpresso.nxp.com/cmsis_pack/repo/NXP.pidx|grep LPC55S69|python -c'import sys; print(sys.stdin.read().rsplit("version=\"", 1)[1].split("\"", 1)[0])')
PACK := NXP.LPC55S69_DFP.$(PACK_VERSION).pack
get-cmsis-pack:
	wget -qq https://mcuxpresso.nxp.com/cmsis_pack/repo/$(PACK) -O ./lpc55s69.pack
