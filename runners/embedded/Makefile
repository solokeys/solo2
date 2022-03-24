### NRF52 device serial ID <-> device type mapping.
### If present, should contain assignments like this:
###
### SERIAL_NRFDK := SERIAL1 SERIAL2 SERIAL3 ...
### SERIAL_PROTO1 := SERIAL4 SERIAL5 ...
###
### (Do LPC55 boards expose something like this?)
-include Makefile.devices

EXTRA_FEATURES ?=

RUST_TRIPLET_NRF52 := thumbv7em-none-eabihf
GNU_TRIPLET_NRF52 := arm-none-eabi

RUST_TRIPLET_LPC55 := thumbv8m.main-none-eabi
GNU_TRIPLET_LPC55 := arm-none-eabi

BOARDS_NRF52 := nrfdk nk3am # proto1
BOARDS_LPC55 := nk3xn # nk3am solo2 okdoe1 lpcxpresso55

.PHONY: all clean build-all

build-all:

clean:
	rm -rf target

define board-targets
.PHONY: build-$(1) clean-$(1)

build-$(1):
	@echo "***** BUILD $(1) *****"
	rm -f target/$(2)/release/$(4)-$(1)
	cargo build --release --target $(2) --features board-$(1) $(EXTRA_FEATURES) \
		--color always 2> build-$(1).err.log \
		|| ( cat build-$(1).err.log; exit 1 )
	cp target/$(2)/release/$(4) ./$(4)-$(1)
	$(3)-nm ./$(4)-$(1) | grep ' [ABbDdRrTt] ' | sort | c++filt > symbols-$(1).txt
	cat $(3)-arch-symbols.txt >> symbols-$(1).txt
	$(3)-objcopy -O ihex ./$(4)-$(1) ./$(4)-$(1).ihex
	$(3)-readelf -l ./$(4)-$(1) | grep LOAD

clean-$(1):
	rm -f ./$(4)-$(1) ./$(4)-$(1).ihex symbols-$(1).txt build-$(1).err.log

build-all: build-$(1)
endef

define nrf-targets
program-nrf-$(1):
	nrfjprog -f NRF52 -s $(1) --sectorerase --program ./nrf-runner-$(2).ihex
	nrfjprog -f NRF52 -s $(1) --reset

reset-nrf-$(1):
	nrfjprog -f NRF52 -s $(1) --chiperase --program ./nrf-runner-$(2).ihex
	nrfjprog -f NRF52 -s $(1) --program fidokeys_lfs_e0000+20000.ihex
	
	# set APPROTECT (disable debugging port - reset with: `nrfjprog -f NRF52 --recover` or --chiperase(?)
	#nrfjprog --memwr 0x10001208 --val 0xFFFFFF00
	
	# set UICR::REGOUT0 to 3v3
	nrfjprog -f NRF52 -s $(1) --memwr 0x10001304 --val 0xfffffffc
	
	# set UICR::NFCPINS to disabled
	nrfjprog -f NRF52 -s $(1) --memwr 0x1000120C --val 0xfffffffe

	nrfjprog -f NRF52 -s $(1) --reset

.PHONY: program-nrf-$(1) reset-nrf-$(1)
endef

$(foreach b,$(BOARDS_NRF52), $(eval $(call board-targets,$(b),$(RUST_TRIPLET_NRF52),$(GNU_TRIPLET_NRF52),nrf-runner)) )
$(foreach b,$(BOARDS_LPC55), $(eval $(call board-targets,$(b),$(RUST_TRIPLET_LPC55),$(GNU_TRIPLET_LPC55),lpc-runner)) )

$(foreach s,$(SERIAL_NRFDK), $(eval $(call nrf-targets,$(s),nrfdk)) )
$(foreach s,$(SERIAL_PROTO1), $(eval $(call nrf-targets,$(s),proto1)) )
$(foreach s,$(SERIAL_NK3AM), $(eval $(call nrf-targets,$(s),nk3am)) )

ifeq ($(words $(NRF_SERIAL)),0)
DEVICES_PRESENT := $(shell nrfjprog -i)
ifeq ($(words $(DEVICES_PRESENT)),1)
$(info Only one NRF device present, defaulting to $(DEVICES_PRESENT).)
NRF_SERIAL := $(DEVICES_PRESENT)
endif
endif

ifeq ($(words $(NRF_SERIAL)),1)
ifneq ($(filter $(NRF_SERIAL),$(SERIAL_NRFDK),$(SERIAL_NK3AM)),)
program: program-nrf-$(NRF_SERIAL)

reset: reset-nrf-$(NRF_SERIAL)
endif
endif
