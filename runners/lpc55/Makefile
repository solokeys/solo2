BOARD := board-lpcxpresso55
DEVELOP := --features $(BOARD),develop
PROVISIONER := --features $(BOARD),develop-provisioner

build-dev:
	cargo build --release $(DEVELOP)

run-dev:
	cargo run --release $(DEVELOP)

run-pro:
	cargo run --release $(PROVISIONER)

bacon:
	bacon


PACK_VERSION := $(shell wget -O - -qq https://mcuxpresso.nxp.com/cmsis_pack/repo/NXP.pidx|grep LPC55S69|python -c'import sys; print(sys.stdin.read().rsplit("version=\"", 1)[1].split("\"", 1)[0])')
PACK := NXP.LPC55S69_DFP.$(PACK_VERSION).pack
get-cmsis-pack:
	wget -qq https://mcuxpresso.nxp.com/cmsis_pack/repo/$(PACK) -O ./lpc55s69.pack

