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
///
/// # Note
/// The syntax to setup multiple copies of a given interchange (for instance,
/// we use this in `trussed` for multi-client) is horrible. Please let the
/// authers know if there's a better way, than the current
/// `interchange!(Name: (Request, Response), 3, [None, None, None])` etc.
#[macro_export]
macro_rules! interchange {
    ($Name:ident: ($REQUEST:ty, $RESPONSE:ty)) => {
        $crate::interchange!($Name: ($REQUEST, $RESPONSE, 1, [None]));
    };

    ($Name:ident: ($REQUEST:ty, $RESPONSE:ty, $N:expr, $Nones:expr)) => {

        // TODO: figure out how to implement, e.g., Clone iff REQUEST
        // and RESPONSE are clone (do not introduce Clone, Debug, etc. trait bounds).
        #[derive(Clone, Debug, PartialEq)]
        pub enum $Name {
            Request($REQUEST),
            Response($RESPONSE),
        }

        impl $Name {
            fn split(i: usize) -> ($crate::Requester<Self>, $crate::Responder<Self>) {
                use core::sync::atomic::AtomicU8;
                use core::mem::MaybeUninit;
                use core::cell::UnsafeCell;

                // TODO(nickray): This turns up in .data section, fix this.
                // static mut INTERCHANGES: [Option<$Name>; $N] = [None; $N];
                static mut INTERCHANGES: [Option<$Name>; $N] = $Nones;
                static mut STATES: [u8; $N] = [0u8; $N];
                unsafe {
                    let mut cell: MaybeUninit<UnsafeCell<&'static mut Option<$Name>>> = MaybeUninit::uninit();

                    // need to pipe everything through an core::cell::UnsafeCell to get past Rust's
                    // aliasing rules (aka the borrow checker) - note that Requester and Responder
                    // both get a &'static mut to the same underlying memory allocation.
                    cell.as_mut_ptr().write(UnsafeCell::new(&mut INTERCHANGES[i]));

                    let state_ref = unsafe { core::mem::transmute::<&u8, &AtomicU8>(&STATES[i]) };

                    (
                        $crate::Requester {
                            interchange: *(*cell.as_mut_ptr()).get(),
                            state: state_ref,
                        },

                        $crate::Responder {
                            interchange: *(*cell.as_mut_ptr()).get(),
                            state: state_ref,
                        },
                    )
                }
            }
        }

        impl $crate::Interchange for $Name {
            type REQUEST = $REQUEST;
            type RESPONSE = $RESPONSE;

            // needs to be a global singleton
            fn claim(i: usize) -> Option<($crate::Requester<Self>, $crate::Responder<Self>)> {
                use core::sync::atomic::{AtomicBool, Ordering};
                // static CLAIMED: [AtomicBool; $N] = [AtomicBool::new(false); $N];
                static CLAIMED: [bool; $N] = [false; $N];//AtomicBool::new(false); $N];
                if unsafe { core::mem::transmute::<bool, AtomicBool>(CLAIMED[i]) }
                    .compare_exchange_weak(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    Some(Self::split(i))
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

