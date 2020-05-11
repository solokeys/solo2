/// Use this macro to generate a pair of RPC pipes for any pair
/// of Request/Response enums you wish to implement.
///
/// ```
/// use interchange::Interchange as _;
/// use interchange::interchange;
/// #[derive(Clone, Debug, PartialEq)]
/// pub enum Request {
///     This(u8, u32),
///     That(i64),
/// }
///
/// #[derive(Clone, Debug, PartialEq)]
/// pub enum Response {
///     Here(u8, u8, u8),
///     There(i16),
/// }
///
/// interchange::interchange! {
///     ThisThatHereThisInterchange: (Request, Response)
/// }
/// ```
#[macro_export]
macro_rules! interchange {
    ($Name:ident: ($REQUEST:ty, $RESPONSE:ty)) => {

        #[derive(Clone, Debug, PartialEq)]
        pub enum $Name {
            // no previous response during initialisation, need a dummy entry
            #[doc(hidden)]
            None,
            Request($REQUEST),
            Response($RESPONSE),
        }

        impl $Name {
            fn split() -> ($crate::RequestPipe<Self>, $crate::ResponsePipe<Self>) {
                pub use core::sync::atomic::AtomicU8;
                static mut INTERCHANGE: $Name = $Name::None;
                static STATE: AtomicU8 = AtomicU8::new($crate::State::Idle as u8);

                unsafe {
                    let mut interchange_cell: core::mem::MaybeUninit<core::cell::UnsafeCell<&'static mut $Name>> = core::mem::MaybeUninit::uninit();
                    // let mut state_cell: core::mem::MaybeUninit<core::cell::UnsafeCell<&'static mut $crate::State>> = core::mem::MaybeUninit::uninit();

                    // need to pipe everything through an core::cell::UnsafeCell to get past Rust's aliasing rules
                    // (aka the borrow checker) - note that $crate::RequestPipe and $crate::ResponsePipe both get `&'static mut`
                    // to the same underlying memory allocation.
                    interchange_cell.as_mut_ptr().write(core::cell::UnsafeCell::new(&mut INTERCHANGE));
                    // state_cell.as_mut_ptr().write(core::cell::UnsafeCell::new(&mut STATE));

                    (
                        $crate::RequestPipe {
                            interchange: *(*interchange_cell.as_mut_ptr()).get(),
                            // state: *(*state_cell.as_mut_ptr()).get(),
                            state_byte: &STATE,
                        },

                        $crate::ResponsePipe {
                            interchange: *(*interchange_cell.as_mut_ptr()).get(),
                            // state: *(*state_cell.as_mut_ptr()).get(),
                            state_byte: &STATE,
                        },

                    )
                }
            }
        }

        impl $crate::Interchange for $Name {
            type REQUEST = $REQUEST;
            type RESPONSE = $RESPONSE;

            // needs to be a global singleton
            fn claim() -> Option<($crate::RequestPipe<Self>, $crate::ResponsePipe<Self>)> {
                use core::sync::atomic::{AtomicBool, Ordering};
                static CLAIMED: AtomicBool = AtomicBool::new(false);
                if CLAIMED
                    .compare_exchange_weak(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    Some(Self::split())
                } else {
                    None
                }
            }

            unsafe fn rq_ref(&self) -> &Self::REQUEST {
                match *self {
                    Self::Request(ref request) => {
                        request
                    }
                    _ => unreachable!(),
                }
            }

            unsafe fn rq_mut(&mut self) -> &mut Self::REQUEST {
                match *self {
                    Self::Request(ref mut request) => {
                        request
                    }
                    _ => unreachable!(),
                }
            }

            unsafe fn rp_ref(&self) -> &Self::RESPONSE {
                match *self {
                    Self::Response(ref response) => {
                        response
                    }
                    _ => unreachable!(),
                }
            }

            unsafe fn rp_mut(&mut self) -> &mut Self::RESPONSE {
                match *self {
                    Self::Response(ref mut response) => {
                        response
                    }
                    _ => unreachable!(),
                }
            }

            fn from_rq(rq: Self::REQUEST) -> Self {
                Self::Request(rq)
            }

            fn from_rp(rp: Self::RESPONSE) -> Self {
                Self::Response(rp)
            }

        }

    }
}

