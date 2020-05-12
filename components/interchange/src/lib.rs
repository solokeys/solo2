#![no_std]
//! Implement a somewhat convenient and somewhat efficient way to perform RPC
//! in an embedded context.
//!
//! The approach is inspired by Go's channels, with the restriction that
//! there is a clear separation into a requester and a responder.
//!
//! Requests may be canceled, which the responder should honour on a
//! best-effort basis.
//!
//! For each pair of `Request` and `Response` types, the macro `interchange!`
//! generates a type that implements the `Interchange` trait.
//!
//! The `Requester` and `Responder` types (to send/cancel requests, and to
//! respond to such demands) are generic with only this one type parameter.
//!
//! ### Example use cases
//! - USB device interrupt handler performs low-level protocol details, hands off
//!   commands from the host to higher-level logic running in the idle thread.
//!   This higher-level logic need only understand clearly typed commands, as
//!   moduled by variants of a given `Request` enum.
//! - `trussed` crypto service, responding to crypto request from apps across
//!   TrustZone for Cortex-M secure/non-secure boundaries.
//! - Request to blink a few lights and reply on button press
//!
//! ```
//! # use interchange::Interchange as _;
//! # use interchange::State;
//! #[derive(Clone, Debug, PartialEq)]
//! pub enum Request {
//!     This(u8, u32),
//!     That(i64),
//! }
//!
//! #[derive(Clone, Debug, PartialEq)]
//! pub enum Response {
//!     Here(u8, u8, u8),
//!     There(i16),
//! }
//!
//! interchange::interchange! {
//!     ExampleInterchange: (Request, Response)
//! }
//!
//! let (mut rq, mut rp) = ExampleInterchange::claim().unwrap();
//!
//! assert!(rq.state() == State::Idle);
//!
//! // happy path: no cancelation
//! let request = Request::This(1, 2);
//! assert!(rq.request(request).is_ok());
//!
//! let request = rp.take_request().unwrap();
//! println!("rp got request: {:?}", &request);
//!
//! let response = Response::There(-1);
//! assert!(!rp.is_canceled());
//! assert!(rp.respond(response).is_ok());
//!
//! let response = rq.take_response().unwrap();
//! println!("rq got response: {:?}", &response);
//!
//! // early cancelation path
//! assert!(rq.request(request).is_ok());
//!
//! let request =  rq.cancel().unwrap().unwrap();
//! println!("responder could cancel: {:?}", &request);
//!
//! assert!(rp.take_request().is_none());
//! assert!(State::Idle == rq.state());
//!
//! // late cancelation
//! assert!(rq.request(request).is_ok());
//! let request = rp.take_request().unwrap();
//!
//! println!("responder could cancel: {:?}", &rq.cancel().unwrap().is_none());
//! assert!(rp.is_canceled());
//! assert!(rp.respond(response).is_err());
//! assert!(rp.acknowledge_cancel().is_ok());
//! assert!(State::Idle == rq.state());
//!
//! ```
//!
//! ### Approach
//! It is assumed that all requests fit in a single `Request` enum, and that
//! all responses fit in single `Response` enum. The macro `interchange!`
//! allocates a static buffer in which either response or request fit, and
//! handles synchronization.
//!
//! An alternative approach would be to use two heapless Queues of length one
//! each for response and requests. The advantage of our construction is to
//! have only one static memory region in use.
//!
//! ### Safety
//! It is possible that this implementation is currently not sound. To be determined!
//!
//! Due to the macro construction, certain implementation details are more public
//! than one would hope for: the macro needs to run in the code of users of this
//! library. We take a somewhat Pythonic "we're all adults here" approach, in that
//! the user is expected to only use the publicly documented API (the ideally private
//! details are hidden from documentation).

use core::sync::atomic::{AtomicU8, Ordering};

mod macros;
// pub mod scratch;

#[repr(u8)]
#[derive(Copy, Clone, PartialEq)]
/// State of the RPC interchange
pub enum State {
    /// The requester may send a new request.
    Idle = 0,
    /// The request is pending either processing by responder or cancelation by requester.
    Requested = 1,
    /// The request is taken by responder, may still be opportunistically canceled by requester.
    Processing = 2,
    /// The responder sent a response.
    Responded = 3,

    #[doc(hidden)]
    CancelingRequested = 4,
    #[doc(hidden)]
    CancelingProcessing = 5,
    /// The requester canceled the request. Responder needs to acknowledge to return to `Idle`
    /// state.
    Canceled = 6,
}

impl PartialEq<u8> for State {
    #[inline]
    fn eq(&self, other: &u8) -> bool {
        *self as u8 == *other
    }
}

impl From<u8> for State {
    fn from(byte: u8) -> Self {
        match byte {
            1 => State::Requested,
            2 => State::Processing,
            3 => State::Responded,

            4 => State::CancelingRequested,
            5 => State::CancelingProcessing,
            6 => State::Canceled,

            _ => State::Idle,
        }
    }
}


/// Do NOT implement this yourself! Use the macro `interchange!`.
pub trait Interchange: Sized {
    type REQUEST: Clone;
    type RESPONSE: Clone;
    /// This is the constructor for a `(Requester, Responder)` pair.
    ///
    /// The first time it is called in the program, it constructs
    /// singleton static resources, thereafter, `None` is returned.
    fn claim() -> Option<(Requester<Self>, Responder<Self>)>;

    #[doc(hidden)]
    unsafe fn rq_ref(&self) -> &Self::REQUEST;
    #[doc(hidden)]
    unsafe fn rp_ref(&self) -> &Self::RESPONSE;
    #[doc(hidden)]
    unsafe fn rq_mut(&mut self) -> &mut Self::REQUEST;
    #[doc(hidden)]
    unsafe fn rp_mut(&mut self) -> &mut Self::RESPONSE;
    #[doc(hidden)]
    fn from_rq(rq: Self::REQUEST) -> Self;
    #[doc(hidden)]
    fn from_rp(rp: Self::RESPONSE) -> Self;
    #[doc(hidden)]
    unsafe fn rq(self) -> Self::REQUEST;
    #[doc(hidden)]
    unsafe fn rp(self) -> Self::RESPONSE;
}

/// Requesting end of the RPC interchange.
///
/// The owner of this end initiates RPC by sending a request.
/// It must then either wait until the responder end responds, upon which
/// it can send a new request again. It does so by periodically checking
/// whether `take_response` is Some. Or it can attempt to cancel,
/// which the responder may or may not honour. For details, see the
/// `cancel` method.
pub struct Requester<I: 'static + Interchange> {
    // todo: get rid of this publicity
    #[doc(hidden)]
    pub interchange: &'static mut Option<I>,
    #[doc(hidden)]
    pub state: &'static AtomicU8,
}

unsafe impl<I: Interchange> Send for Requester<I> {}

/// Processing end of the RPC interchange.
///
/// The owner of this end must eventually reply to any requests made to it.
/// In case there is a cancelation of the request, this must be acknowledged instead.
pub struct Responder<I: 'static + Interchange> {
    #[doc(hidden)]
    pub interchange: &'static mut Option<I>,
    #[doc(hidden)]
    pub state: &'static AtomicU8,
}

unsafe impl<I: Interchange> Send for Responder<I> {}

impl<I: Interchange> Requester<I> {

    #[inline]
    /// Current state of the interchange.
    ///
    /// Note that this is a snapshot, and the responder may change
    /// this state between calls.
    pub fn state(&self) -> State {
        State::from(self.state.load(Ordering::Acquire))
    }

    /// Send a request to the responder.
    ///
    /// If the RPC state is `Idle`, this always succeeds, else calling
    /// is a logic error and the request is returned.
    pub fn request(&mut self, request: I::REQUEST) -> Result<(), I::REQUEST> {
        if State::Idle == self.state.load(Ordering::Acquire) {
            *self.interchange = Some(Interchange::from_rq(request));
            self.state.store(State::Requested as u8, Ordering::Release);
            Ok(())
        } else {
            Err(request)
        }
    }

    /// Attempt to cancel a request.
    ///
    /// If the responder has not taken the request yet, this succeeds and returns
    /// the request.
    ///
    /// If the responder has taken the request (is processing), we succeed and return None.
    ///
    /// In other cases (`Idle` or `Reponsed`) there is nothing to cancel and we fail.
    pub fn cancel(&mut self) -> Result<Option<I::REQUEST>, ()> {

        // we canceled before the responder was even aware of the request.
        if self.state.compare_exchange(
            State::Requested as u8,
            State::CancelingRequested as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).is_ok() {
            if let Some(thing) = self.interchange.take() {
                self.state.store(State::Idle as u8, Ordering::Release);
                return Ok(Some(unsafe { thing.rq() } ));
            }
            unreachable!();
        }

        // we canceled after the responder took the request, but before they answered.
        if self.state.compare_exchange(
            State::Processing as u8,
            State::CancelingProcessing as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).is_ok() {
            // this may not yet be None in case the responder switched state to
            // Processing but did not take out the request yet.
            // assert!(self.interchange.is_none());
            self.state.store(State::Canceled as u8, Ordering::Release);
            return Ok(None);
        }

        Err(())
    }

    /// Look for a response.
    ///
    /// If the responder has sent a response, we return it.
    // It is a logic error to call this method if we're Idle or Canceled, but
    // it seems unnecessary to model this.
    pub fn take_response(&mut self) -> Option<I::RESPONSE> {
        if self.state.compare_exchange(
            State::Responded as u8,
            State::Idle as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).is_ok() {
            if let Some(thing) = self.interchange.take() {
                return Some(unsafe { thing.rp() } );
            }
            unreachable!();
        }

        None
    }

}

impl<I: Interchange> Responder<I> {

    #[inline]
    pub fn state(&self) -> State {
        State::from(self.state.load(Ordering::Acquire))
    }

    // If there is a request waiting, take it out
    pub fn take_request(&mut self) -> Option<I::REQUEST> {
        if self.state.compare_exchange(
            State::Requested as u8,
            State::Processing as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).is_ok() {
            if let Some(thing) = self.interchange.take() {
                return Some(unsafe { thing.rq() } );
            }
            unreachable!();
        }

        None
    }

    // Check if requester attempted to cancel
    pub fn is_canceled(&self) -> bool {
        self.state.load(Ordering::SeqCst) == State::Canceled as u8
    }

    // Acknowledge a cancel, thereby setting Interchange to Idle state again.
    //
    // It is a logic error to call this method if there is no pending cancellation.
    pub fn acknowledge_cancel(&self) -> Result<(), ()> {
        if self.state.compare_exchange(
            State::Canceled as u8,
            State::Idle as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).is_ok() {
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn respond(&mut self, response: I::RESPONSE) -> Result<(), I::RESPONSE> {
        if State::Processing == self.state.load(Ordering::Acquire) {
            *self.interchange = Some(I::from_rp(response));
            if self.state.compare_exchange(
                State::Processing as u8,
                State::Responded as u8,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ).is_ok() {
                return Ok(());
            } else {
                // requester canceled in the mean time
                if let Some(thing) = self.interchange.take() {
                    return Err(unsafe { thing.rp() } );
                }
                unreachable!();
            }
        }

        // logic error
        Err(response)
    }

}

// pub fn claim() -> Option<(Requester, Responder)> {
//     static CLAIMED: AtomicBool = AtomicBool::new(false);
//     if CLAIMED
//         .compare_exchange_weak(false, true, Ordering::AcqRel, Ordering::Acquire)
//         .is_ok()
//     {
//         static mut INTERCHANGE: Option<Interchange> = None;
//         static STATE: AtomicU8 = AtomicU8::new(State::Idle as u8);

//         use core::mem::MaybeUninit;
//         use core::cell::UnsafeCell;
//         unsafe {
//             let mut cell: MaybeUninit<UnsafeCell<&'static mut Option<Interchange>>> = MaybeUninit::uninit();
//             cell.as_mut_ptr().write(UnsafeCell::new(&mut INTERCHANGE));
//             Some((
//                 Requester {
//                     interchange: *(*cell.as_mut_ptr()).get(),
//                     state: &STATE,
//                 },

//                 Responder {
//                     interchange: *(*cell.as_mut_ptr()).get(),
//                     state: &STATE,
//                 },
//             ))
//         }
//     } else {
//         None
//     }
// }
