pub use embedded_hal::blocking::rng::Read as RngRead;
pub use crate::store::Store;

// This is the same trick as in "store.rs",
// replacing generic parameters with associated types
// and a macro.
pub unsafe trait Board {
    type R: RngRead;
    type S: Store;

    fn rng(&mut self) -> &mut Self::R;
    fn store(&self) -> Self::S;
}

#[macro_export]
macro_rules! board { (
    $BoardName:ident,
    R: $Rng:ty,
    S: $Store:ty,
) => {
    pub struct $BoardName {
        rng: $Rng,
        store: $Store,
    }

    impl $BoardName {
        pub fn new(rng: $Rng, store: $Store) -> Self {
            Self { rng, store }
        }
    }

    unsafe impl $crate::board::Board for $BoardName {
        type R = $Rng;
        type S = $Store;

        fn rng(&mut self) -> &mut Self::R {
            &mut self.rng
        }

        fn store(&self) -> Self::S {
            self.store
        }
    }
}}


