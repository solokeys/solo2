pub mod clock_controller;
pub mod traits;
pub mod trussed;
pub mod types;

/*
   Rust being ridiculous, episode #14728.

   For brevity and because it stacks nicely, we would like to write
   the following - it captures exactly what we want and it's elegant
   and perfectly readable:

#[cfg_attr(feature = "board-nk3am", path = "board_nk3am.rs")]
#[cfg_attr(feature = "board-solo2", path = "board_solo2.rs")]
#[cfg_attr(feature = "board-nk3xn", path = "board_nk3xn.rs")]
pub mod board;

   However, due to this PR[1], the presence of a path attribute changes
   the way that nested modules (i.e. those inside the module with that
   attribute) are looked up. With the attribute, rustc doesn't make
   "board-nk3xn" and friends "directory owners", so all nested modules
   are expected to be in the same directory.

   And even though there's ample documentation for the module subsystem
   and the effects of the path attribute[2], this aspect isn't mentioned.
   Even worse (and also quite customary for Rust), there's a bug report[3]
   open since 2019 for adding exactly that.

   There's even a simple fix! See below for the magic special case.

[1]: https://github.com/rust-lang/rust/pull/37602
[2]: https://doc.rust-lang.org/reference/items/modules.html
[3]: https://github.com/rust-lang/reference/issues/573
 */

// modules with path attribute *are* directory owners if their path
// refers to a 'mod.rs'
#[cfg_attr(feature = "board-nk3am", path = "board_nk3am/mod.rs")]
#[cfg_attr(feature = "board-solo2", path = "board_solo2/mod.rs")]
#[cfg_attr(feature = "board-nk3xn", path = "board_nk3xn/mod.rs")]
pub mod board;

pub fn init_bootup() {
	unsafe { types::DEVICE_UUID.copy_from_slice(&lpc55_hal::uuid()); };
}
