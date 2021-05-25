# board

This implements `trussed::Platform` for some LPC55S69 boards.

The main ones are:
- [LPCXpresso55S69][lpcxpresso], the official development board by NXP
- [Solo 2][solo2], the new security key by SoloKeys
- NK3XN, the new [Nitrokey 3A NFC][nk3an] and [Nitrokey 3C NFC][nk3cn] devices
- NK3AM, the new [Nitrokey 3A Mini][nk3am] device

These can be selected via the features `board-lpcxpresso55`, `board-solo2`,
`board-nk3xn` and `board-nk3am`, respectively.

It is more convenient to develop on the LPC55S69-EVK as it has `PIO0_5`, the `ISP0` pin, exposed.
This allows forcing boot-to-bootloader, so you can't realy brick yourself until you start playing
with secure boot settings. Also of course, it has the debugger embedded, vs. having
to somehow attach it to the Solo 2's Tag-Connect headers. However, there is no NFC
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
[solo2]: https://solo2.dev
[nk3an]: https://shop.nitrokey.com/shop/product/nk3an-nitrokey-3a-nfc-147
[nk3cn]: https://shop.nitrokey.com/shop/product/nk3cn-nitrokey-3c-nfc-148
[nk3am]: https://shop.nitrokey.com/shop/product/nk3am-nitrokey-3a-mini-149
