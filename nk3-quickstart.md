# Nitrokey 3 Firmware Development Quickstart Guide

*See also: [Solo 2 Getting Started](https://hackmd.io/@solokeys/solo2-getting-started)*

## Requirements

* A Nitrokey 3 prototype
* Python
* A current Rust installation and rustup, see [rustup.rs](https://rustup.rs)
  * Every six weeks (for every new Rust release): `rustup update`

## Clone Firmware Repository

Clone the firmware repository, currently [nitrokey/solo2](https://github.com/nitrokey/solo2) (upstream: [solokeys/solo2](https://github.com/solokeys/solo2)):

```
$ git clone https://github.com/nitrokey/solo2
$ cd solo2
```

## Install Dependencies

Install the dependencies as described in the [readme](./README.md) and in the
[Solo 2 Getting Started guide](https://solo2.dev), for example:

```
$ sudo apt-get install llvm clang libclang-dev gcc-arm-none-eabi libc6-dev-i386
$ cargo install flip-link
$ rustup target add thumbv8m.main-none-eabi
```

Also install these dependencies for building the firmware image:

```
$ cargo install cargo-binutils
$ rustup component add llvm-tools-preview
```

And install [mboot](https://github.com/molejar/pyMBoot) for flashing the firmware (if you don’t use a debugger):

```
$ pip install mboot
```

Optionally, add and activate the mboot udev roles so that you don’t need root privileges for flashing:

```
$ curl https://raw.githubusercontent.com/molejar/pyIMX/master/udev/90-imx-sdp.rules > /etc/udev/rules.d/90-imx-sdp.rules
$ sudo udevadm control --reload-rules
```

## Compile Firmware

Compile the firmware and create the firmware image:

```
cd runners/lpc55
cargo objcopy --release --features board-nk3xn,develop -- -O binary firmware-nk3xn.bin
```

Select the features according to the device you use and the settings you want to activate:

* board selection
  * `board-nk3xn` for NK3AN and NK3CN
  * `board-nk3am` for NK3AM
* settings
  * `no-buttons`: disable the touch button and always confirm actions
  * `no-encrypted-storage`: don’t encrypt the flash chip (currently required)
  * `no-reset-time-window`: allow resetting FIDO authenticator (and possibly others) even after 10s uptime
  * `develop` = `no-buttons` + `no-encrypted-storage` + `no-reset-time-window`

If you set the `BOARD` environment variable or update the `FEATURES` variable
in the `Makefile` accordingly, you can also use `make build-dev` to compile the
firmware or `make objcopy-dev` to create the firmware image.

## Flash Firmware

Insert the Nitrokey 3 prototype (or activate the reset button) while holding the bootloader button (or otherwise connecting the BL and GND pads). You should see information about the device when running `mboot info`:

```
$ mboot info
```

Flash the firmware using `mboot write`:

```
$ mboot erase --mass
$ mboot write firmware-nk3xn.bin
```

(If you did not install the udev rules, these commands require root privilges.)

You can also compile and flash the firmware in one go by running `make flash-dev`.

## Test Firmware

* FIDO2
  * `nitropy fido2 list` and `nitrop2 fido2 verify`
  * <https://webauthn.bin.coffee/>
    * Create Credential + Get Assertion
  * `pip install 'fido2~=0.9' && python tests/basic.py`

## Debugging

The Nitrokey 3 board can be debugged using SWD. The SWD interface is exposed over the GND, (SW)DIO and (SW)CLK pins. An external debugger is required, for example [LPC-Link2](https://www.embeddedartists.com/products/lpc-link2/).

### Preparing the Connection

* Remove one of the connectors from a 2x5 SWD cable.
* Solder the SWD cables to the pads on the Nitrokey board:
  * Cable 2 (SWDIO): DIO
  * Cable 3 (GND): GND
  * Cable 4 (SWDCLK): CLK
* Connect the cable to the J7 socket on the debugger.

### J-LINK

#### Install the J-LINK firmware

(See also the [NXP Debug Probe Firmware Programming Guide](https://www.nxp.com/docs/en/supporting-information/Debug_Probe_Firmware_Programming.pdf)).

* Close JP1 and connect the board.
* Check the output of `lsusb -d 1366:0101`. You should see a SEGGER J-Link PLUS device. If the device is present, you can skip this section. If it is not present, you have to update the debugger firmware as described in this section.
* Download and install `lpcscrypt`.
* Check the [Segger LPC-Link2 site](https://www.segger.com/lpc-link-2.html) for updated firmware images.
* Disconnect the board, open JP1 and reconnect the board to the computer.
* Run `/usr/local/lpcscrypt/scripts/program_JLINK`.
* Disconnect the board, close JP1 reconnect the board to the computer. Now the device should appear in the `lsusb` output.

#### Running the Debugger

* Close JP2.
* Install the [JLink Software and Documentation pack](https://www.segger.com/downloads/jlink/#J-LinkSoftwareAndDocumentationPack).
* Run `JLinkGDBServer -strict -device LPC55S69 -if SWD -vd`.

### CMSIS

(Untested.)

Install the CMSIS firmware image instead of the J-Link firmware image and then:

* Install `pyocd`.
* Run `pyocd gdb -u 1042163622 -t lpc55s69`.

### Troubleshooting

* Probe is detected but not target:
  * Make sure that JP2 is closed.
