## 🐝

[![Documentation][docs-image]][docs-link]

SoloKeys reimagined.

To make pcsclite on Linux work with the Bee, on Debian copy `Info.plist` to `/etc/libccid_Info.plist`.

[docs-image]: https://img.shields.io/badge/docs-book-green?style=flat-square
[docs-link]: https://solo-bee.netlify.com

## Building

### Prerequisites

On Ubuntu or Debian:

```
sudo apt-get install llvm clang
```

Install [Rust and Cargo](https://www.rust-lang.org/tools/install) for your system.


### Boards

Head to [platforms/lpc55/board](platforms/lpc55/board) for an overview on the possible embedded platforms the firmware
currently supports.

### Compiling

Head to [platforms/lpc55/runner](platforms/lpc55/runner) to get started, and try `make build-dev`, which compiles
the entire firmware bundle using features convenient for getting started.

With `make run-dev`, it will try to connect to a GDB server to flash and run the firmware.

To enable logs, you can change the feature flags on each crate.  Then logs will be output via semihosting to your SWD debugger.
```
# Enable logs on the root crate and a few of the local crate dependencies.
cargo run --release --features board-lpcxpresso55,log-all,fido-authenticator/log-all,hid-dispatch/log-info,ctap-types/log-all
```

#### License

<sup>`solo-bee` is licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.</sup>
<br>
<sub>Any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.</sub>
