

BUILD_ID ?= $(BUILD_PROFILE)-$(BOARD)
CFG_PATH ?= profiles/$(BUILD_PROFILE).toml

ifeq ($(filter nk3am proto1 nrfdk,$(BOARD)), )
  SOC = lpc55
else
  SOC = nrf52
endif

# also RUST_TARGET
TARGET = $(shell python -c 'import toml; print(toml.load("$(CFG_PATH)")["platform"]["target"])')
# also binutils-prefix
GNU_TARGET = arm-none-eabi

### @TODO: ugly deluxe ? toml bash parser? dirty grep? 
CFG_VAL_BOARDS = $(shell python -c 'import toml; print(" ".join(toml.load("$(CFG_PATH)")["platform"]["boards"]))')
CFG_VAL_SOC = $(shell python -c 'import toml; print(toml.load("$(CFG_PATH)")["platform"]["soc"])')

# input files and outputs
SRCS = $(shell find . -name "*.rs" | grep -v '/target/')
OUT = runner-$(BUILD_ID).bin
OUT_IHEX = $(OUT).ihex
RAW_OUT = $(SOC)_runner

# util fn/paths
LOG ?= build-$(BUILD_ID).err.log
SYMBOLS ?= symbols-$(BUILD_ID).txt

# feature definition
FEATURES ?= develop
BUILD_FEATURES := board-$(BOARD) $(FEATURES)
# assemble comma-seperated list to pass to `cargo build`
delim = ,
space := $(null) #
COMMA_FEATURES = $(subst $(space),$(delim),$(BUILD_FEATURES))


.PHONY: list build reset program clean clean-all check-env set-vars

# default target -> just build all "shortcuts"
all: build-nrfdk build-nk3am build-proto1 build-nk3xn

help:
	###############################################################################
	###############################################################################
	## To build a specific 'board' & 'build_profile' combination:
	## $ make build BUILD_PROFILE=blubla BOARD=foobar
	##
	## Or build using one of the pre-defined "target shortcuts" (see `make list`):
	## $ make build-proto1
	##
	## AND pass EXTRA_FEATURES="A B C" for build feature-variants
	## $ make build-nk3am FEATURES=complete
	###############################################################################
	###############################################################################

GET_TARGET = $(shell echo $(@) | cut -d '-' -f 1)

%-nrfdk:
	$(MAKE) $(GET_TARGET) BUILD_PROFILE=nrf52 BOARD=nrfdk
%-nk3am:
	$(MAKE) $(GET_TARGET) BUILD_PROFILE=nrf52 BOARD=nk3am
%-proto1:
	$(MAKE) $(GET_TARGET) BUILD_PROFILE=nrf52 BOARD=proto1
%-nk3xn:
	$(MAKE) $(GET_TARGET) BUILD_PROFILE=lpc55 BOARD=nk3xn
	
ifeq ($(BUILD_ID),)
ifneq ($(shell stat $(CFG_PATH) | head -n1),File: $(CFG_PATH))
 $(error Configuration: $(CFG_PATH) not found!)
 $(shell exit 1)
endif
endif

$(OUT): $(SRCS) build
$(OUT_IHEX): $(OUT) 

check-var-%:
	@if [ -z '${${*}}' ]; then echo 'FAIL: var: $* required!!!' && exit 1; fi

%-banner:
	@echo "******************************************************************************************"
	@echo "**** TARGET:   $(shell printf %18s $(GET_TARGET)) | BINARY:   $(OUT)(.ihex)"
	@echo "**** BOARD:    $(shell printf %18s $(BOARD)) | SOC:      $(SOC)"
	@echo "**** PROFILE:  $(shell printf %18s $(BUILD_PROFILE)) | BUILD_ID: $(BUILD_ID)"
	@echo "**** FEATURES: $(BUILD_FEATURES)"
	@echo "******************************************************************************************"

list:
	@echo "Available:"
	@echo "  BOARD(s): $(shell find src/ -name 'board_*' | xargs -n1 basename | sed -E 's/board_([^.]*)(.rs)?/\1/g' | xargs)"
	@echo "  BUILD_PROFILE(s): $(shell ls profiles -1 | sed -e 's/\.toml//g')"
	@echo "  Makefile shortcuts: nrfdk, proto1, nk3xc, nk3am "
	@echo ""
	@echo "  Usage: make <target>-<something> e.g., build-nk3am, reset-proto1, ..." 

clean-all: 
	rm -f ./$(OUT) ./$(OUT_IHEX) $(SYMBOLS) $(LOG)
	rm -f *.log runner-*-*.ihex runner-*-*.bin symbols-*-*.txt runner-*-*.ihex
	rm -rf ./target
	rm -f firmware.hex mbr.hex bootloader.hex

###############################################################################
###############################################################################

define fidokeys_lfs
fidokeys_lfs_$(1)+$(2).ihex:
	$(MAKE) -C deployment clean
	$(MAKE) -C deployment fs.hex FS_BASE=0x$(1) FS_SIZE=0x$(2)
	cp deployment/fs.hex $$@
endef

$(eval $(call fidokeys_lfs,d8000,14000))

build: build-banner $(SRCS) check-var-BOARD check-var-BUILD_PROFILE check-var-SOC

	rm -f target/$(TARGET)/release/$(RAW_OUT)

	BUILD_PROFILE=$(BUILD_PROFILE) BOARD=$(BOARD) \
		cargo build --release --target $(TARGET) \
		--features $(COMMA_FEATURES) \
		--color always 2> $(LOG) || ( cat $(LOG); exit 1 )

	cp target/$(TARGET)/release/$(RAW_OUT) ./$(OUT)

	$(GNU_TARGET)-nm ./$(OUT) | grep ' [ABbDdRrTt] ' | sort | c++filt > $(SYMBOLS) 
	
	cat $(GNU_TARGET)-arch-symbols.txt >> $(SYMBOLS)

	$(GNU_TARGET)-objcopy -O ihex ./$(OUT) ./$(OUT_IHEX)
	$(GNU_TARGET)-readelf -l ./$(OUT) | grep LOAD

clean: clean-banner check-var-BOARD check-var-BUILD_PROFILE
	rm -f ./$(OUT) ./$(OUT_IHEX) target/$(TARGET)/release/$(RAW_OUT) $(SYMBOLS) $(LOG)

reset: reset-banner fidokeys_lfs_d8000+14000.ihex check-var-BOARD check-var-BUILD_PROFILE $(OUT_IHEX) $(OUT) 
	nrfjprog -f NRF52 --chiperase --program $(OUT_IHEX)
	nrfjprog -f NRF52 --program fidokeys_lfs_d8000+14000.ihex
	
	# UICR
	## UICR::REGOUT0 to 3v3
	nrfjprog -f NRF52 --memwr 0x10001304 --val 0xfffffffd
	## UICR::NFCPINS to disabled
	nrfjprog -f NRF52 --memwr 0x1000120C --val 0xfffffffe
	## UICR::APPROTECT (only --recover will revive debug-access)
	#nrfjprog -f NRF52 --memwr 0x10001208 --val 0xFFFFFF00
	
	nrfjprog -f NRF52 --reset

program: program-banner check-var-BOARD check-var-BUILD_PROFILE $(OUT_IHEX) $(OUT) 
	nrfjprog -f NRF52 --program $(OUT_IHEX) --sectorerase
	
	nrfjprog --reset

############################################
############################################   TODO:

reset-bootloader: nrf-bootloader/mbr.hex nrf-bootloader/bootloader.hex
	nrfjprog -f NRF52 --recover
	nrfjprog -f NRF52 --program nrf-bootloader/mbr.hex --sectorerase
	nrfjprog -f NRF52 --program nrf-bootloader/bootloader.hex --sectorerase
	
	# UICR
	## UICR::REGOUT0 to 3v3
	nrfjprog -f NRF52 --memwr 0x10001304 --val 0xfffffffd
	## UICR::NFCPINS to disabled
	nrfjprog -f NRF52 --memwr 0x1000120C --val 0xfffffffe
	## UICR::APPROTECT (only --recover will revive debug-access)
	#nrfjprog -f NRF52 --memwr 0x10001208 --val 0xFFFFFF00
	
	nrfjprog -f NRF52 --reset

program-bootloader:	nrf-bootloader/mbr.hex nrf-bootloader/bootloader.hex
	nrfjprog -f NRF52 --program nrf-bootloader/mbr.hex --sectorerase
	nrfjprog -f NRF52 --program nrf-bootloader/bootloader.hex --sectorerase
	nrfjprog -f NRF52 --reset



fw_signed_update.zip: fw-assemble-dfu
fw_unsigned.hex: $(OUT_IHEX)
	cp $< $@
	
fw-assemble-dfu: $(OUT_IHEX) $(OUT) $(SIGNING_KEY) fw_unsigned.hex
	#bash sign.sh $(FIRMWARE_MONO_VERSION) $(SIGNING_KEY) fw_unsigned.hex fw_signed_update.zip

fw-upload-dfu: $(OUT_IHEX) $(OUT) fw_signed_update.zip
	#bash update.sh /dev/ttyACM1 fw_signed_update.zip



