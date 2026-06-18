.PHONY: build build-dk build-dk-nfc-only build-solo build-dk-test flash flash-jtag flash-jtag-test flash-set-key bacon clean-dfu

# ── Paths ─────────────────────────────────────────────────────────────────────
ELF      := ../../target/thumbv7em-none-eabihf/release/runner-nrf52840dk
APP_HEX  := $(ELF).hex

DFU_DIR  := dfu
# Prefer prod artifacts if they exist (made by `make flash-set-key`),
# fall back to the committed dev key + bootloader.
DFU_KEY  := $(if $(wildcard $(DFU_DIR)/prod.pem),$(DFU_DIR)/prod.pem,$(DFU_DIR)/debug.pem)
BOOT_HEX := $(if $(wildcard $(DFU_DIR)/bootloader_prod.hex),$(DFU_DIR)/bootloader_prod.hex,$(DFU_DIR)/bootloader.hex)
# MBR is a separate hex (~4 KB) — not bundled in the bootloader's own hex.
# Lives at 0x0; jumps to bootloader at UICR.NRFFW[0] = 0xF4000.
MBR_HEX  := $(DFU_DIR)/mbr.hex

# nrfutil DFU port — override on cmdline if auto-detect picks the wrong one.
# The DK exposes two tty.usbmodem* ports for J-Link OB's own VCP (serial
# starts with `001050...` on PCA10056). Filter those out so we hit the
# chip's USB CDC — that's the bootloader (VID 0x1915 PID 0x521F) when in
# DFU mode, or the app's CDC when in app mode.
NRF_PORT ?= $(shell ls /dev/tty.usbmodem* 2>/dev/null | grep -v "tty.usbmodem001050" | head -1)

# Nordic's Python nrfutil — `pip3 install --user --break-system-packages nrfutil`.
# Adafruit's fork dropped the `pkg`/`settings` subcommands we need; use upstream.
# nrfutil 5.2.0 is the last working version; 6.x has dependencies (pc-ble-driver-py)
# without wheels for modern Python. On Python 3.14 you also need the patches in
# dfu/README.md applied to nordicsemi/dfu/signing.py.
PATH := $(HOME)/Library/Python/3.14/bin:$(PATH)
NRFUTIL := nrfutil

APP_VER ?= 1
HW_VER  := 52

# ── Build targets ─────────────────────────────────────────────────────────────
# `build` builds whatever the default Cargo features pick (= board-dk).
# `build-dk` and `build-solo` force a specific board; subsequent flash targets
# are board-agnostic — they flash whatever's currently in $(ELF).
build: build-dk

# Default DK build: CTAPHID + USB-CCID + NFC. CCID is on by default — phones
# do FIDO2-over-NFC (incl. registration) fine with it; only strict bench
# readers (ACR1252) go flaky from the USBD/NFCT EasyDMA contention.
build-dk:
	cargo build --release --no-default-features --features board-dk,ccid

# NFC-only DK build (no USB-CCID): max NFC reliability, solid on strict readers
# too. Use when you need contactless to be rock-solid and don't need contact-CCID.
build-dk-nfc-only:
	cargo build --release --no-default-features --features board-dk

build-solo:
	cargo build --release --no-default-features --features board-solo

# Builds with `test-up-control` on, which adds the `UP_CONTROL` static
# the host-side test harness writes via probe-rs to drive user-presence
# approvals. Otherwise identical to `build-dk` (CCID on). NEVER ship this build.
build-dk-test:
	cargo build --release --no-default-features --features board-dk,ccid,test-up-control,no-reset-time-window

# ── DFU upgrade (the common case) ─────────────────────────────────────────────
# Assumes bootloader is already installed via `make flash-jtag`. App must be
# in DFU mode — either via admin "enter DFU" command, or hold Button 4 at
# reset to enter via the bootloader's built-in trigger.
flash:
	@if [ ! -f "$(ELF)" ]; then echo "ERROR: no ELF at $(ELF) — run 'make build-dk' or 'make build-solo' first"; exit 1; fi
	arm-none-eabi-objcopy -O ihex $(ELF) $(APP_HEX)
	$(NRFUTIL) pkg generate \
	    --hw-version $(HW_VER) \
	    --sd-req 0 \
	    --application-version $(APP_VER) \
	    --application $(APP_HEX) \
	    --key-file $(DFU_KEY) \
	    /tmp/firmware.zip
	@if [ -z "$(NRF_PORT)" ]; then \
	    echo "ERROR: no /dev/tty.usbmodem* found. Is the device in DFU mode?"; \
	    exit 1; \
	fi
	@echo "Using port $(NRF_PORT) (override with NRF_PORT=...)"
	$(NRFUTIL) dfu usb-serial -pkg /tmp/firmware.zip -p $(NRF_PORT) -b 115200

# ── Initial install via JLink (one-shot per device, also for recovery) ────────
# The default build runs CTAPHID + CCID over USB and NFC together — every app
# over both transports. No coexistence tradeoff on the nRF52840.
flash-jtag: $(BOOT_HEX)
	@if [ ! -f "$(ELF)" ]; then echo "ERROR: no ELF at $(ELF) — run 'make build-dk' or 'make build-solo' first"; exit 1; fi
	cp $(ELF) $(ELF).elf
	arm-none-eabi-objcopy -O ihex $(ELF) $(APP_HEX)
	$(NRFUTIL) settings generate \
	    --family NRF52840 \
	    --application $(APP_HEX) \
	    --application-version $(APP_VER) \
	    --bootloader-version 1 \
	    --bl-settings-version 2 \
	    /tmp/settings.hex
	@printf 'r\nhalt\nerase\nloadfile %s\nloadfile %s\nloadfile %s\nloadfile /tmp/settings.hex\nr\ng\nq\n' \
	    "$(MBR_HEX)" "$(BOOT_HEX)" "$(APP_HEX)" > /tmp/flash-jtag.jlink
	JLinkExe -device nRF52840_xxAA -if SWD -speed 4000 -autoconnect 1 \
	         -CommanderScript /tmp/flash-jtag.jlink

# Build the test-up-control variant + flash via J-Link. The companion
# host-side tests (see `runners/pc/scripts/device_test.sh`) drive UP via
# `probe-rs write` against `UP_CONTROL`. NEVER ship.
flash-jtag-test: build-dk-test
	$(MAKE) flash-jtag

# ── Generate a fresh prod keypair + rebuild bootloader to trust it ────────────
# Run once. Output: dfu/prod.pem + dfu/bootloader_prod.hex (both gitignored).
# Subsequent `make flash` and `make flash-jtag` automatically prefer these.
# Requires NRF5_SDK_ROOT to point at an extracted nRF5 SDK 17.1.0.
flash-set-key:
	@if [ -z "$(NRF5_SDK_ROOT)" ]; then \
	    echo "ERROR: set NRF5_SDK_ROOT to your nRF5 SDK 17.1.0 directory"; \
	    exit 1; \
	fi
	$(NRFUTIL) keys generate $(DFU_DIR)/prod.pem
	@echo "Wrote $(DFU_DIR)/prod.pem"
	$(NRFUTIL) keys display --key pk --format code --out_file $(DFU_DIR)/dfu_public_key.c $(DFU_DIR)/prod.pem
	@echo "Wrote $(DFU_DIR)/dfu_public_key.c"
	# Backup SDK pubkey + linker, swap in ours, build with the same tighter
	# layout as bootloader.hex (origin 0xF4000, length 0xA000), restore after.
	cp $(NRF5_SDK_ROOT)/examples/dfu/dfu_public_key.c $(NRF5_SDK_ROOT)/examples/dfu/dfu_public_key.c.bak
	cp $(DFU_DIR)/dfu_public_key.c $(NRF5_SDK_ROOT)/examples/dfu/dfu_public_key.c
	cp $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc/open_bootloader_gcc_nrf52.ld \
	    $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc/open_bootloader_gcc_nrf52.ld.bak
	sed -i.bak 's|FLASH (rx) : ORIGIN = 0xe0000, LENGTH = 0x1e000|FLASH (rx) : ORIGIN = 0xf4000, LENGTH = 0xa000|' \
	    $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc/open_bootloader_gcc_nrf52.ld
	# nRF5 SDK was tested with GCC ~9; modern (15+) is stricter. Disable -Werror.
	sed -i.werror.bak 's/CFLAGS += -Wall -Werror/CFLAGS += -Wall/' \
	    $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc/Makefile
	cd $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc && \
	    $(MAKE) clean GNU_INSTALL_ROOT=/opt/homebrew/bin/ GNU_VERSION=15.2.1 GNU_PREFIX=arm-none-eabi && \
	    $(MAKE) GNU_INSTALL_ROOT=/opt/homebrew/bin/ GNU_VERSION=15.2.1 GNU_PREFIX=arm-none-eabi
	cp $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc/_build/nrf52840_xxaa.hex \
	    $(DFU_DIR)/bootloader_prod.hex
	# Restore SDK to original.
	mv $(NRF5_SDK_ROOT)/examples/dfu/dfu_public_key.c.bak $(NRF5_SDK_ROOT)/examples/dfu/dfu_public_key.c
	mv $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc/open_bootloader_gcc_nrf52.ld.bak \
	    $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc/open_bootloader_gcc_nrf52.ld
	mv $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc/Makefile.werror.bak \
	    $(NRF5_SDK_ROOT)/examples/dfu/open_bootloader/pca10056_usb/armgcc/Makefile
	@echo "Done. Now run 'make flash-jtag' to install the prod bootloader + app."

clean-dfu:
	rm -f $(DFU_DIR)/prod.pem $(DFU_DIR)/bootloader_prod.hex
	@echo "Removed prod artifacts. Subsequent flashes use the committed debug key."

bacon:
	bacon
