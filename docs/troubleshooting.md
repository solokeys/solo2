# Troubleshooting Guide

This guide contains solutions for common issues during development.

## Compilation

### Compilation Issues

If the firmware from the repository no longer compiles, make sure that you are using the correct Rust version.  Generally, we are using the latest stable Rust release.  If that does not work, you might want to use the stable Rust version at the time of the last commit (see the [Rust changelog][] for the release dates).

[Rust changelog]: https://github.com/rust-lang/rust/blob/master/RELEASES.md

## Debugging

### `arm-none-eabi-gdb` Not Found

`cargo run` per default uses the `arm-none-eabi-gdb` binary (see `runners/lpc55/.cargo/config`).  On some systems, this executable is called differently, for example `gdb-mulitarch` on Debian.  The easist persistent solution for this problem is to create a link with that name.
