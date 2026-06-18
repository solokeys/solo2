# DFU artifacts

Files in this directory back the `make flash` and `make flash-jtag` targets.

## One-time host setup

The Makefile uses Nordic's Python `nrfutil` (NOT the `adafruit-nrfutil` fork —
adafruit dropped the `pkg`/`settings` subcommands we need).

```sh
pip3 install --user --break-system-packages nrfutil
```

The latest `nrfutil` that installs on modern Python is **5.2.0**; 6.x has a
native dep (`pc-ble-driver-py`) without wheels for current Python/arm64.

### Python 3.14 patches (skip on 3.12 and earlier)

`nrfutil 5.2.0` ships a lot of Python-2-era code that breaks on 3.14. The
sweep below covers everything the `make flash-jtag` + `make flash` paths
hit in practice. Run all of these against your venv's `nordicsemi/`
(replace `$VENV` with your venv path, e.g. `solo2/venv`):

```sh
NS=$VENV/lib/python3.14/site-packages/nordicsemi

# xrange → range, .iteritems() → .items() and friends, .tostring() → .tobytes()
find $NS -name "*.py" -exec sed -i '' \
    -e 's/xrange/range/g' \
    -e 's/\.iteritems()/.items()/g' \
    -e 's/\.iterkeys()/.keys()/g' \
    -e 's/\.itervalues()/.values()/g' \
    -e 's/\.tostring()/.tobytes()/g' \
    {} \;

# signing.py: pem write mode + hex/bytes confusion
F=$NS/dfu/signing.py
sed -i '' \
    -e 's|with open(filename, "w") as sk_file:|with open(filename, "wb") as sk_file:|' \
    -e "s|sk_hex = \"\".join(c.encode('hex') for c in self.sk.to_string())|sk_hex = self.sk.to_string().hex()|" \
    -e 's|sk_hexlify = binascii.hexlify(self.sk.to_string())|sk_hexlify = binascii.hexlify(self.sk.to_string()).decode()|' \
    -e 's|vk_hexlify = binascii.hexlify(vk.to_string())|vk_hexlify = binascii.hexlify(vk.to_string()).decode()|' \
    -e 's|vk_hex = binascii.hexlify(vk.to_string())|vk_hex = binascii.hexlify(vk.to_string()).decode()|' \
    "$F"

# intelhex/__init__.py: int division, removed (int, long), bytes-to-str confusion,
# dict.keys() not having .sort()
F=$NS/dfu/intelhex/__init__.py
sed -i '' \
    -e 's/(int, long)/(int,)/g' \
    -e 's|return asstr(self._tobinarray_really(start, end, pad, size).tobytes())|return self._tobinarray_really(start, end, pad, size).tobytes()|' \
    -e 's|addresses = self._buf.keys()|addresses = sorted(self._buf.keys())|g' \
    -e 's|^\([[:space:]]*\)addresses\.sort()$|\1pass  # already sorted|g' \
    "$F"

# nrfhex.py: int division producing float
F=$NS/dfu/nrfhex.py
sed -i '' 's|(size + (word_size - 1)) / word_size|(size + (word_size - 1)) // word_size|' "$F"

# intelhex/compat.py: asbytes() rejecting bytearray
F=$NS/dfu/intelhex/compat.py
sed -i '' 's|if isinstance(s, bytes):|if isinstance(s, (bytes, bytearray)):|' "$F"
# (also wrap return as bytes(s) — safer for bytearray input)

# init_packet_pb.py: protobuf bytes field rejecting str
F=$NS/dfu/init_packet_pb.py
# (manual: wrap boot_validation_bytes[i] in `if isinstance(_bv, str): _bv = _bv.encode('latin1')`)

# dfu_transport_serial.py: map() not iterable + bytes already int + int division
F=$NS/dfu/dfu_transport_serial.py
sed -i '' \
    -e 's|+ map(ord, struct.pack(.<H., self.prn)))|+ list(struct.pack("<H", self.prn)))|' \
    -e 's|+ map(ord, struct.pack(.<L., size)))|+ list(struct.pack("<L", size)))|' \
    -e 's|self.dfu_adapter.send_message(map(ord, to_transmit))|self.dfu_adapter.send_message(list(to_transmit))|' \
    -e 's|(self.mtu-1)/2|(self.mtu-1)//2|g' \
    "$F"
```

A couple sites need manual edits (the exact `sed` patterns are fragile around
newlines and quotes). See `dfu/init_packet_pb.py` near "boot_validation":
inside the `for i, x in enumerate(boot_validation_type):` loop, replace the
single `boot_validation.append(...)` line with:

```python
                _bv = boot_validation_bytes[i]
                if isinstance(_bv, str):
                    _bv = _bv.encode('latin1')
                boot_validation.append(pb.BootValidation(type=x.value, bytes=_bv))
```

And `dfu/intelhex/compat.py` near `asbytes`: ensure it returns `bytes(s)` not
`s` so a bytearray gets coerced to bytes (or the protobuf complains).

## Files in this directory (committed unless noted)

- `mbr.hex` — Master Boot Record (4 KB at 0x0). From the SDK at
  `components/softdevice/mbr/hex/mbr_nrf52_2.4.1_mbr.hex`. The MBR is what the
  CPU jumps into at reset; it then jumps to the bootloader (address read from
  UICR.NRFFW[0]). Required at install time but never updated via DFU.

## Committed (in git)

- `bootloader.hex` — Nordic Open Bootloader (USB CDC, no SoftDevice), built
  from nRF5 SDK 17.1.0 `examples/dfu/open_bootloader/pca10056_usb` against
  `dfu_public_key.c` below. Combined hex: includes MBR at 0x0 + bootloader
  at 0xE0000 + UICR's `NRFFW[0]` pointer.
- `debug.pem` — ECDSA P-256 private key matching `dfu_public_key.c`.
  **Anyone with this repo can sign firmware updates.** That's intentional —
  `make flash` works out of the box for everyone. Don't use this in prod.
- `dfu_public_key.c` — the bootloader's verification key, in the SDK 17 `pk[64]`
  format. Burned into `bootloader.hex` at build time.

## Gitignored (per-machine, optional)

- `prod.pem` — your private key. Generated by `make flash-set-key`.
- `bootloader_prod.hex` — bootloader rebuilt against the matching public key.
  Also from `make flash-set-key`.

When prod files exist, both `make flash` and `make flash-jtag` use them
instead of the committed debug versions. `make clean-dfu` removes them.

## Workflows

### Default (debug key)

```sh
make flash-jtag         # one-shot per device — installs bootloader + app via JLink
# (later, after editing code)
# Put device in DFU mode: hold Button 4 while pressing the reset button,
#                         OR send the admin 'enter DFU' command over USB.
make flash              # builds, signs with debug.pem, DFUs over USB CDC
```

### Production (your own key)

```sh
export NRF5_SDK_ROOT=~/Downloads/nRF5_SDK_17.1.0_ddde560
make flash-set-key      # one-shot — generates prod.pem + bootloader_prod.hex
make flash-jtag         # one-shot per device — installs prod bootloader + app
# (later)
make flash              # signs with prod.pem from now on
```

If you lose `prod.pem`: USB DFU is broken until you `make clean-dfu` +
`make flash-set-key` + `make flash-jtag` (which wipes IFS via chip erase).
JLink is always your recovery path.

## How `make flash` finds the device

The Makefile picks `$(NRF_PORT)` via `ls /dev/tty.usbmodem* | head -1` by
default. If that picks the wrong port (e.g., your app's CDC instead of the
bootloader's), override:

```sh
make flash NRF_PORT=/dev/tty.usbmodem0006839481591
```

Bootloader USB IDs: VID `0x1915` PID `0x521F` (Nordic).

## Memory layout

```
0x000_0000 ── MBR ────────── 0x000_1000   (4 KB)      from bootloader.hex
0x000_1000 ── App ────────── 0x007_4000   (460 KB)    from this firmware
0x007_4000 ── IFS ────────── 0x00F_4000   (512 KB)    littlefs2
0x00F_4000 ── Bootloader ─── 0x00F_E000   (40 KB)     from bootloader.hex
0x00F_E000 ── MBR params ─── 0x00F_F000   (4 KB)      bootloader-managed
0x00F_F000 ── BL settings ── 0x010_0000   (4 KB)      bootloader-managed
```

The bootloader was rebuilt against a tighter linker script (origin moved
from 0xE0000 to 0xF4000, length 0x1E000 → 0xA000) to free 80 KB for the
app. Bootloader binary is ~38 KB; we leave 40 KB allocated for headroom.

## Rebuilding the debug bootloader

If you ever need to regenerate `bootloader.hex` (e.g., updated `dfu_public_key.c`):

```sh
export NRF5_SDK_ROOT=~/Downloads/nRF5_SDK_17.1.0_ddde560
cp dfu/dfu_public_key.c $NRF5_SDK_ROOT/examples/dfu/dfu_public_key.c   # backup the SDK's first
cd $NRF5_SDK_ROOT/examples/dfu/open_bootloader/pca10056_usb/armgcc
# nRF5 SDK was tested with GCC 9; modern GCC 15+ requires loosening:
sed -i.bak 's/CFLAGS += -Wall -Werror/CFLAGS += -Wall/' Makefile
make clean GNU_INSTALL_ROOT=/opt/homebrew/bin/ GNU_VERSION=15.2.1 GNU_PREFIX=arm-none-eabi
make       GNU_INSTALL_ROOT=/opt/homebrew/bin/ GNU_VERSION=15.2.1 GNU_PREFIX=arm-none-eabi
cp _build/nrf52840_xxaa.hex /path/to/runners/nrf52840dk/dfu/bootloader.hex
```
