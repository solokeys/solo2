// SPDX-License-Identifier: MIT
//
// build.rs — link the vendored static archives. Both .a's plus their
// bindgen output live under ./vendor and are committed; this build does
// no C compilation and runs no bindgen. Regenerate via `make regen`
// when upstream sources change.

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_path = manifest.join("vendor/lib");
    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=static=nrfx_nfct");
    println!("cargo:rustc-link-lib=static=nfc_t4t");
    println!("cargo:rerun-if-changed=vendor/lib/libnrfx_nfct.a");
    println!("cargo:rerun-if-changed=vendor/lib/libnfc_t4t.a");
    println!("cargo:rerun-if-changed=vendor/nrfx_nfct_bindings.rs");
    println!("cargo:rerun-if-changed=vendor/nfc_t4t_bindings.rs");
}
