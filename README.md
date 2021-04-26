## 🐝

[![Documentation][docs-image]][docs-link]

SoloKeys Solo 2 monorepo.

**WARNING WARNING WARNING**
- **EARLY PRERELEASE.**
- **NOT FOR PUBLIC USE.**
- **PULL REQUESTS / ISSUES ONLY AFTER INITIAL CONSULTATION WITH THE TEAM.**



For **technical questions and discussions**: https://github.com/solokeys/solo2/discussions

For **product / crowdfunding questions and discussions**: https://github.com/solokeys/kickstarter2021/discussions

For general discussion:

- visit our Keybase chat: https://keybase.io/team/solokeys.public
- visit one of our Matrix rooms: https://matrix.to/#/+solokeys:matrix.org



To repeat: Please **do not** open issues/PRs that are not technical issues / firmware bugs



To make pcsclite on Linux work with the Bee, on Debian copy `Info.plist` to `/etc/libccid_Info.plist`.

[docs-image]: https://img.shields.io/badge/docs-book-green?style=flat-square
[docs-link]: https://solo-bee.netlify.com

## Getting Started

### Prerequisites

- Install [Rust and Cargo](https://www.rust-lang.org/tools/install) for your system.

- Install dependencies such as clang, llvm, arm-none-eabi-gdb

- Get and prepare hardware
- Run `make build-dev`.

For more information: <https://hackmd.io/@solokeys/solo2-getting-started>.
Please **do not** send PRs to expand on getting started generalities, just edit the linked HackMD directly.


### Boards

Head to [runnners/lpc55/board](runners/lpc55/board) for an overview on the possible embedded platforms the firmware
currently supports.

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

<sup>This software is licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.</sup>
<br>
<sub>Any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.</sub>
