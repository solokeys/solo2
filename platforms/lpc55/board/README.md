# board

This implements `trussed::Platform` for some LPC55S69 boards.

The main ones are:
- [LPCXpresso55S69][lpcxpresso], the official development board by NXP
- [Solo V2][solov2], the new security key by SoloKeys

These can be selected via the features `board-lpcxpresso55` and `board-solov2`,
respectively.

It is more convenient to develop on the LPC55S69-EVK as it has `PIO0_5`, the `ISP0` pin, exposed.
This allows forcing boot-to-bootloader, so you can't realy brick yourself until you start playing
with secure boot settings. Also of course, it has the debugger embedded, vs. having
to somehow attach it to the Solo V2's Tag-Connect headers. However, there is no NFC
on the dev kit, two buttons instead of three, and the RGB led doesn't work properly.

There is also some low-effort support for the (cheap!) [OKdo E1][okdoe1]. This board however
does not have a crystal soldered by default, and hence cannot run the USB HS peripheral.
This also means it has one less bidirectional USB endpoint; keep this in mind when
USB classes stop working. It can be selected via the `board-okdoe1`, which patches
the `board-lpcxpresso55` implementation.

For development, on the main boards, we recommend using Segger's JLinkGDBServer:
```
JLinkGDBServer -strict -device LPC55S69 -if SWD -vd
```

For OKdo E1, there's no JLink support, modify `.cargo/config` in the runner to use `pyocd.gdb`, and try your luck with
```
pyocd gdbserver
```
Flashing firmware is *much* slower.

[lpcxpresso]: https://www.nxp.com/design/development-boards/lpcxpresso-boards/lpcxpresso55s69-development-board:LPC55S69-EVK
[okdoe1]: https://www.okdo.com/p/okdo-e1-development-board/
[solov2]: https://solokeys.com
