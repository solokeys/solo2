BOARD ?= nk3xn

FEATURES := board-${BOARD}

ifeq "${DEVELOP}" "1"
ifeq "${PROVISIONER}" "1"
FEATURES := ${FEATURES},develop-provisioner
else
FEATURES := ${FEATURES},develop
endif
else ifeq "${PROVISIONER}" "1"
FEATURES := ${FEATURES},provisioner
endif

ifeq "${PROVISIONER}" "1"
FILENAME_PREFIX := provisioner-
else
FILENAME_PREFIX := firmware-
endif

ifeq "${DEVELOP}" "1"
FILENAME_SUFFIX := -develop
else
FILENAME_SUFFIX := 
endif

FILENAME := ${FILENAME_PREFIX}${BOARD}${FILENAME_SUFFIX}.bin

.PHONY: ci
ci:
	$(MAKE) build 
	$(MAKE) objcopy
	$(MAKE) size

.PHONY: build
build:
	cargo build --release --features $(FEATURES)

.PHONY: run
run:
	cargo run --release --features $(FEATURES)

.PHONY: objcopy
objcopy:
	cargo objcopy --release --features $(FEATURES) -- -O binary "${FILENAME}"

.PHONY: flash
flash: objcopy
	mboot erase --mass
	mboot write "${FILENAME}"

.PHONY: size
size:
	cargo size --release --features $(FEATURES)

bacon:
	bacon

jlink:
	../../scripts/bump-jlink
	JLinkGDBServer -strict -device LPC55S69 -if SWD -vd


PACK_VERSION := $(shell wget -O - -qq https://mcuxpresso.nxp.com/cmsis_pack/repo/NXP.pidx|grep LPC55S69|python -c'import sys; print(sys.stdin.read().rsplit("version=\"", 1)[1].split("\"", 1)[0])')
PACK := NXP.LPC55S69_DFP.$(PACK_VERSION).pack
get-cmsis-pack:
	wget -qq https://mcuxpresso.nxp.com/cmsis_pack/repo/$(PACK) -O ./lpc55s69.pack

