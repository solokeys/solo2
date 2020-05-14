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
///     ExampleInterchange: (Request, Response)
/// }
/// ```
#[macro_export]
macro_rules! interchange {
    ($Name:ident: ($REQUEST:ty, $RESPONSE:ty)) => {

        // TODO: figure out how to implement, e.g., Clone iff REQUEST
        // and RESPONSE are clone (do not introduce Clone, Debug, etc. trait bounds).
        #[derive(Clone, Debug, PartialEq)]
        pub enum $Name {
            Request($REQUEST),
            Response($RESPONSE),
        }

        impl $Name {
            fn split() -> ($crate::Requester<Self>, $crate::Responder<Self>) {
                use core::sync::atomic::AtomicU8;
                use core::mem::MaybeUninit;
                use core::cell::UnsafeCell;

                // TODO(nickray): This turns up in .data section, fix this.
                static mut INTERCHANGE: Option<$Name> = None;
                static STATE: AtomicU8 = AtomicU8::new($crate::State::Idle as u8);

                unsafe {
                    let mut cell: MaybeUninit<UnsafeCell<&'static mut Option<$Name>>> = MaybeUninit::uninit();

                    // need to pipe everything through an core::cell::UnsafeCell to get past Rust's
                    // aliasing rules (aka the borrow checker) - note that Requester and Responder
                    // both get a &'static mut to the same underlying memory allocation.
                    cell.as_mut_ptr().write(UnsafeCell::new(&mut INTERCHANGE));

                    (
                        $crate::Requester {
                            interchange: *(*cell.as_mut_ptr()).get(),
                            state: &STATE,
                        },

                        $crate::Responder {
                            interchange: *(*cell.as_mut_ptr()).get(),
                            state: &STATE,
                        },
                    )
                }
            }
        }

        impl $crate::Interchange for $Name {
            type REQUEST = $REQUEST;
            type RESPONSE = $RESPONSE;

            // needs to be a global singleton
            fn claim() -> Option<($crate::Requester<Self>, $crate::Responder<Self>)> {
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

            unsafe fn rq(self) -> Self::REQUEST {
                match self {
                    Self::Request(request) => {
                        request
                    }
                    _ => unreachable!(),
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

            unsafe fn rp(self) -> Self::RESPONSE {
                match self {
                    Self::Response(response) => {
                        response
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

