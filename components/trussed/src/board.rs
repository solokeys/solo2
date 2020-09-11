pub use embedded_hal::blocking::rng::Read as RngRead;
pub use crate::store::Store;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum UserPresenceIndication {
    Strong,
    Light,
    None,
    CantTell,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Status {
    Idle,
    WaitingForUserPresence,
    Processing,
    Error,
}

pub trait UserInterface {
    /// Check if the user has indicated their presence so as to give
    /// consent to an action.
    fn check_user_presence(&mut self) -> UserPresenceIndication;

    /// Set the state of Trussed to give potential feedback to the user.
    fn set_status(&mut self, status: Status);
}

pub trait UpTime {
    /// Return the duration since startup.
    fn uptime(&mut self) -> core::time::Duration;
}

// This is the same trick as in "store.rs",
// replacing generic parameters with associated types
// and a macro.
pub unsafe trait Board {
    type R: RngRead;
    type S: Store;
    type UI: UserInterface;
    type UT: UpTime;

    fn rng(&mut self) -> &mut Self::R;
    fn store(&self) -> Self::S;
    fn user_interface(&mut self) -> &mut Self::UI;
    fn uptime(&mut self) -> &mut Self::UT;
}

#[macro_export]
macro_rules! board { (
    $BoardName:ident,
    R: $Rng:ty,
    S: $Store:ty,
    UT: $UpTime:ty,
    UI: $UserInterface:ty,
) => {

    pub struct $BoardName {
        rng: $Rng,
        store: $Store,
        uptime: $UpTime,
        user_interface: $UserInterface,
    }

    impl $BoardName {
        pub fn new(rng: $Rng, store: $Store, uptime: $UpTime, user_interface: $UserInterface) -> Self {
            Self { rng, store, uptime, user_interface }
        }
    }

    unsafe impl $crate::board::Board for $BoardName {
        type R = $Rng;
        type S = $Store;
        type UI = $UserInterface;
        type UT = $UpTime;

        fn user_interface(&mut self) -> &mut Self::UI {
            &mut self.user_interface
        }

        fn uptime(&mut self) -> &mut Self::UT {
            &mut self.uptime
        }

        fn rng(&mut self) -> &mut Self::R {
            &mut self.rng
        }

        fn store(&self) -> Self::S {
            self.store
        }
    }
}}


