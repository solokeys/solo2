
Hardware Abstraction Layer (HAL) for [NXP LPC55][nxp-lpc55] series [Cortex-M33][cortex-m33] microcontrollers,
written in Rust.

[![Build Status][github-action-image]][github-action-link]
[![crates.io][crates-image]][crates-link]
![LICENSE][license-image]
[![Documentation][docs-image]][docs-link]
[![Documentation (master)][docs-master-image]][docs-master-link]

LPC55 HAL provides a high-level interface to the features of these LPC55 family of MCUs, which is safe,
convenient and efficient. It leverages Rust's type system to prevent common mistakes, such as attempting
to use an uninitialized peripheral; these will be caught by compile-time errors.

This library implements the [`embedded-hal`][embedded-hal], a collection of traits intended to abstract
over platform-dependencies, allowing firmware and drivers to be quite portable.

It also implements the [`usb-device`][usb-device] framework.

Moreover, this library is compatible with the Cortex-M implementation of [RTIC][cortex-m-rtic],
a concurrency framework supporting preemptive multitasking with minimal footprint.

## Status

Very much work-in-progress!

Current peripherals that mostly work:
- I2C, SPI, Serial (with all pins)
- USB FS device

Next up will be:
- Flash (towards [littleFS][littlefs])
- CASPER (towards even faster [`salty`][salty])

See also the low-level companion library [LPC55S6x PAC][lpc55s6x-pac].

This HAL is intended to work with `cortex-m-rtic` v0.5.

## Documentation

The API documentation is located at <https://docs.rs/lpc55-hal>.

In addition, `make fetch-docs` downloads various vendor-supplied documentation:

- [LPC55S6x Data Sheet][datasheet]
- [LPC55 User Manual UM11126][usermanual] (requires an NXP account)
- [LPC55S6x Errata][errata]
- [Cortex-M33 Generic User Guide][genericuserguide]
- [LPCXpresso55S69 Development Board User Manual][evkusermanual] (requires an NXP account)

## Examples

The intention of the [`examples/`][examples] is to showcase the functionality of this library.

They run on the [LPCXpresso55S69][lpcxpresso55s69] development board.

After flashing [J-Link firmware][jlink-fw] on the on-board LPCXpresso V2 debugger:

```bash
# in one terminal
make jlink

# in another terminal
make run-example EXAMPLE=rtic_led  # or any other example
```

## Setup
To install rust, follow the install instructions from here: [https://rust-embedded.github.io/book/intro/install.html][rust_install_manual]

You also need following targets:

- thumbv8m.main-none-eabi
- thumbv8m.main-none-eabihf

Just use following command:

```
  rustup target add thumbv8m.main-none-eabi
  rustup target add thumbv8m.main-none-eabihf
```

## License

[Apache-2.0][apache2-link] or [MIT][mit-link].

Parts of the code are directly lifted from the [LPC8xx HAL][lpc8xx-hal], others
from the various [STM32 HALs][stm32-rs].

[//]: # "links"

[nxp-lpc55]: https://www.nxp.com/products/processors-and-microcontrollers/arm-microcontrollers/general-purpose-mcus/lpc5500-cortex-m33:LPC5500_SERIES
[cortex-m33]: https://developer.arm.com/ip-products/processors/cortex-m/cortex-m33
[embedded-hal]: https://lib.rs/embedded-hal
[usb-device]: https://lib.rs/usb-device
[cortex-m-rtic]: https://lib.rs/cortex-m-rtic
[lpc55s6x-pac]: https://lib.rs/lpc55s6x-pac
[lpc8xx-hal]: https://github.com/lpc-rs/lpc8xx-hal
[stm32-rs]: https://github.com/stm32-rs
[littlefs]: https://github.com/ARMmbed/littlefs
[salty]: https://github.com/nickray/salty
[examples]: https://github.com/nickray/lpc55-hal/tree/main/examples
[lpcxpresso55s69]: https://www.nxp.com/products/processors-and-microcontrollers/arm-microcontrollers/general-purpose-mcus/lpc5500-cortex-m33/lpcxpresso55s69-development-board:LPC55S69-EVK
[jlink-fw]: https://www.segger.com/products/debug-probes/j-link/models/other-j-links/lpcxpresso-on-board/

[crates-image]: https://img.shields.io/crates/v/lpc55-hal.svg?style=flat-square
[crates-link]: https://crates.io/crates/lpc55-hal
[github-action-image]: https://github.com/lpc55/lpc55-hal/workflows/build/badge.svg?branch=main
[github-action-link]: https://github.com/lpc55/lpc55-hal/actions
[docs-image]: https://docs.rs/lpc55-hal/badge.svg?style=flat-square
[docs-link]: https://docs.rs/lpc55-hal
[docs-master-image]: https://img.shields.io/badge/docs-master-blue?style=flat-square
[docs-master-link]: https://lpc55-hal.netlify.com

[license-image]: https://img.shields.io/badge/license-Apache2.0%2FMIT-blue.svg??style=flat-square
[apache2-link]: https://spdx.org/licenses/Apache-2.0.html
[mit-link]: https://spdx.org/licenses/MIT.html

[datasheet]: https://www.nxp.com/docs/en/data-sheet/LPC55S6x.pdf
[usermanual]: https://www.nxp.com/webapp/Download?colCode=UM11126
[errata]: https://www.nxp.com/docs/en/errata/ES_LPC55S6x.pdf
[genericuserguide]: https://static.docs.arm.com/100235/0004/arm_cortex_m33_dgug_100235_0004_00_en.pdf
[evkusermanual]: https://www.nxp.com/webapp/Download?colCode=UM11158
[rust_install_manual]: https://rust-embedded.github.io/book/intro/install.html
