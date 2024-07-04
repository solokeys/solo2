## 🐝 SoloKeys Solo 2 monorepo.

This repository contains the "runner" for the firmware on Solo 2 devices.

Please note that many components have been contributed to the [Trussed GitHub Organization][trussed-dev]:

- Framework: https://github.com/trussed-dev/trussed
- FIDO Authenticator: https://github.com/trussed-dev/fido-authenticator
- PIV Authenticator: https://github.com/trussed-dev/piv-authenticator

Such code is co-maintained with Nitrokey, who have contributed extensions and other improvements.

We plan to release a new version of the firmware, incorporating these changes.

[trussed-dev]: https://github.com/trussed-dev

## Support

For support with purchased devices, please reach out to hello@solokeys.com.

To repeat: Please **do not** open issues/PRs that are not technical issues / firmware bugs.

## Getting Started

### Prerequisites

- Install [Rust and Cargo](https://www.rust-lang.org/tools/install) for your system.

- Install dependencies such as clang, llvm, arm-none-eabi-gdb, flip-link

- Get and prepare hardware
- Run `make build-dev`.

For more information: <https://hackmd.io/@solokeys/solo2-getting-started>.
Please **do not** send PRs to expand on getting started generalities, just edit the linked HackMD directly.


### Boards

Head to [runnners/lpc55/board](runners/lpc55/board) for an overview on the possible embedded platforms the firmware currently supports.

### Compiling

Head to [runners/lpc55](runners/lpc55) to get started, and try `make build-dev`, which compiles
the entire firmware bundle using features convenient for getting started.

With `make run-dev`, it will try to connect to a GDB server to flash and run the firmware.
One way to run a GDB server is `JLinkGDBServer -strict -device LPC55S69 -if SWD -vd`

To enable logs, you can change the feature flags on each crate.  Then logs will be output via semihosting to your SWD debugger.
```
# Enable logs on the root crate and a few of the local crate dependencies.
cargo run --release --features board-lpcxpresso55,log-all,fido-authenticator/log-all,ctaphid-dispatch/log-info,ctap-types/log-all
```

#### License

Solo is fully open source.

All software, unless otherwise noted, is dual licensed under [Apache 2.0](LICENSE-APACHE) and [MIT](LICENSE-MIT).
You may use Solo software under the terms of either the Apache 2.0 license or MIT license.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

All hardware, unless otherwise noted, is licensed under [CERN-OHL-S](https://github.com/solokeys/solo2-hw/blob/main/LICENSE.txt).
You may use Solo hardware under the terms of the CERN-OHL-S license.

All documentation, unless otherwise noted, is licensed under [CC-BY-SA](https://creativecommons.org/licenses/by-sa/4.0/).
You may use Solo documentation under the terms of the CC-BY-SA 4.0 license.
