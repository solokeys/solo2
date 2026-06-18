// SPDX-License-Identifier: MIT
//
//! `nrf-nfc` — nRF52840 NFC stack.
//!
//! Two modules, each a thin Rust layer over a vendored static archive:
//!
//! - [`nrfx_nfct`] — chip-layer driver (Nordic's MIT-licensed nrfx_nfct.c
//!   pre-compiled as `libnrfx_nfct.a`).
//! - [`nfc_t4t`] — ISO 14443-4 Type 4 Tag library (Nordic's closed-source
//!   `libnfc_t4t.a`) plus the Rust platform-glue the library calls back
//!   into.
//!
//! Both .a's and their bindgen outputs live in `vendor/`; see
//! `vendor/README.md` for provenance and `Makefile` for `make regen`.

#![no_std]
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

pub mod nfc_t4t;
pub mod nrfx_nfct;
