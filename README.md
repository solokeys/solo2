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


### Compiling

To quickly build and run on the LPC55 development board, you can run the following.

```rust
cd app/
cargo run --release --bin rtic --features board-lpcxpresso,logging/none
```

It will try to connect to a GDB server to program the target.  We recommend running a JLink GDB server that connects to the target.

Note you need to specific either the `board-lpcxpresso` or the `board-prototype` feature to pick what board you are compiling on.

To enable logs, you can change the feature flags on each crate.  Then logs will be output via semihosting to your SWD debugger.

```
# Enable logs on the root crate and a few of the local crate dependencies.
cargo run --release --bin rtic --features board-prototype,log-all,fido-authenticator/log-all,hid-dispatch/log-all,ctap-types/log-all
```

#### License

<sup>`solo-bee` is licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.</sup>
<br>
<sub>Any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.</sub>
