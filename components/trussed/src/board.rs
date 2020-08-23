pub use embedded_hal::blocking::rng::Read as RngRead;
pub use crate::store::Store;
pub use trussed_board::buttons::{Press, Edge};
pub use trussed_board::rgb_led::RgbLed;

// This is the same trick as in "store.rs",
// replacing generic parameters with associated types
// and a macro.
pub unsafe trait Board {
    type L: RgbLed;
    type R: RngRead;
    type S: Store;
    type T: Press + Edge;

    fn buttons(&mut self) -> &mut Option<Self::T>;
    fn led(&mut self) -> &mut Self::L;
    fn rng(&mut self) -> &mut Self::R;
    fn store(&self) -> Self::S;
}

#[macro_export]
macro_rules! board { (
    $BoardName:ident,
    L: $Led:ty,
    R: $Rng:ty,
    S: $Store:ty,
    T: $ThreeButtons:ty,
) => {

    pub struct $BoardName {
        led: $Led,
        rng: $Rng,
        store: $Store,
        buttons: Option<$ThreeButtons>,
    }

    impl $BoardName {
        pub fn new(led: $Led, rng: $Rng, store: $Store, buttons: Option<$ThreeButtons>) -> Self {
            Self { led, rng, store, buttons }
        }
    }

    unsafe impl $crate::board::Board for $BoardName {
        type L = $Led;
        type R = $Rng;
        type S = $Store;
        type T = $ThreeButtons;

        fn buttons(&mut self) -> &mut Option<Self::T> {
            &mut self.buttons
        }

        fn led(&mut self) -> &mut Self::L {
            &mut self.led
        }

        fn rng(&mut self) -> &mut Self::R {
            &mut self.rng
        }

        fn store(&self) -> Self::S {
            self.store
        }
    }
}}


