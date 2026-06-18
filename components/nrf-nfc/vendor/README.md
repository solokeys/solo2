# Vendored NFC artifacts

Pre-built artifacts the `nrf-nfc` crate links against. Both `.a`'s and
their bindgen outputs are committed so the normal build needs no nrfx
clone, no CMSIS clone, and no C cross-compiler.

## libnrfx_nfct.a (chip layer)

Compiled from Nordic's MIT-licensed
[nrfx](https://github.com/NordicSemiconductor/nrfx) NFCT + TIMER
drivers. Source commits and build flags are pinned in `../Makefile`.

| File | Description |
|---|---|
| `lib/libnrfx_nfct.a` | nrfx_nfct.c + nrfx_timer.c, compiled for thumbv7em-none-eabihf at -O2 |
| `nrfx_nfct_bindings.rs` | bindgen output for nrfx_nfct.h + nrf_nfct.h |

## libnfc_t4t.a (Type 4 Tag library)

Verbatim copy of Nordic's closed-source Type-4-Tag library (ISO 14443-4
ISO-DEP — anti-collision, RATS/ATS, I-block chaining, WTX) from
[nrfconnect/sdk-nrfxlib](https://github.com/nrfconnect/sdk-nrfxlib) at
commit `529012899ffb2aa8ef69cbbb315eaf2848737aca`.

| File | Source path in nrfxlib |
|---|---|
| `lib/libnfc_t4t.a` | `nfc/lib/cortex-m4/hard-float/libnfc_t4t.a` |
| `include/nfc_t4t_lib.h` | `nfc/include/nfc_t4t_lib.h` |
| `include/nrf_nfc_errno.h` | `nfc/include/nrf_nfc_errno.h` |
| `LICENSE` | top-level `LICENSE` (Nordic 5-Clause) |
| `nfc_t4t_bindings.rs` | bindgen output for nfc_t4t_lib.h |

The library's `nfc_platform.h` C-callback contract is implemented in
Rust at [`../src/nfc_t4t/nfc_platform.rs`](../src/nfc_t4t/nfc_platform.rs);
the header itself is not vendored.

`include/` is only consumed at regen time (by bindgen-cli), not by
the normal build.

## Regenerating

When bumping upstream versions, from this crate's directory run
`make regen` (requires `arm-none-eabi-gcc` and `bindgen-cli`). The
artifacts under this directory get rewritten in place; commit them.
