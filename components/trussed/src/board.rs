pub use embedded_hal::blocking::rng::Read as RngRead;
pub use crate::store::Store;
pub use trussed_board::buttons::{Press, Edge};

// This is the same trick as in "store.rs",
// replacing generic parameters with associated types
// and a macro.
pub unsafe trait Board {
    type R: RngRead;
    type S: Store;
    type T: Press + Edge;

    fn rng(&mut self) -> &mut Self::R;
    fn store(&self) -> Self::S;
    fn buttons(&mut self) -> &mut Option<Self::T>;
}

#[macro_export]
macro_rules! board { (
    $BoardName:ident,
    R: $Rng:ty,
    S: $Store:ty,
    T: $ThreeButtons:ty,
) => {

    pub struct $BoardName {
        rng: $Rng,
        store: $Store,
        buttons: Option<$ThreeButtons>,
    }

    impl $BoardName {
        pub fn new(rng: $Rng, store: $Store, buttons: Option<$ThreeButtons>) -> Self {
            Self { rng, store, buttons }
        }
    }

    unsafe impl $crate::board::Board for $BoardName {
        type R = $Rng;
        type S = $Store;
        type T = $ThreeButtons;

        fn buttons(&mut self) -> &mut Option<Self::T> {
            &mut self.buttons
        }

        fn rng(&mut self) -> &mut Self::R {
            &mut self.rng
        }

        fn store(&self) -> Self::S {
            self.store
        }
    }
}}


