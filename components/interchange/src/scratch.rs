use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[repr(u8)]
#[derive(Copy, Clone, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    This(u8, u32),
    That(i64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Response {
    Here(u8, u8, u8),
    There(i16),
}

pub enum Interchange {
    Request(Request),
    Response(Response),
}

// pub enum GenericInterchange<REQUEST, RESPONSE> {
//     Request(REQUEST),
//     Response(RESPONSE),
// }

// pub struct GenericRequester<REQUEST: 'static, RESPONSE: 'static> {
//     #[doc(hidden)]
//     pub interchange: &'static mut Option<GenericInterchange<REQUEST, RESPONSE>>,
//     #[doc(hidden)]
//     pub state: &'static AtomicU8,
// }

pub struct Requester
{
    #[doc(hidden)]
    pub interchange: &'static mut Option<Interchange>,
    #[doc(hidden)]
    pub state: &'static AtomicU8,
}

unsafe impl Send for Requester {}

// pub struct GenericResponder<REQUEST: 'static, RESPONSE: 'static> {
//     #[doc(hidden)]
//     pub interchange: &'static mut Option<GenericInterchange<REQUEST, RESPONSE>>,
//     #[doc(hidden)]
//     pub state: &'static AtomicU8,
// }

pub struct Responder
{
    #[doc(hidden)]
    pub interchange: &'static mut Option<Interchange>,
    #[doc(hidden)]
    pub state: &'static AtomicU8,
}

unsafe impl Send for Responder {}

impl Requester {

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
    // pub fn request(&mut self, request: I::REQUEST) -> Result<(), ()> {
    // pub fn request(&mut self, request: I::REQUEST) -> bool {
    pub fn request(&mut self, request: Request) -> Result<(), Request> {
        if State::Idle == self.state.load(Ordering::Acquire) {
            *self.interchange = Some(Interchange::Request(request));
            self.state.store(State::Requested as u8, Ordering::Release);
            // Ok(())
            // true
            Ok(())
        } else {
            // Err(Error::LogicError)
            // false
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
    pub fn cancel(&mut self) -> Result<Option<Request>, ()> {

        // we canceled before the responder was even aware of the request.
        if self.state.compare_exchange(
            State::Requested as u8,
            State::CancelingRequested as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).is_ok() {
            if let Some(Interchange::Request(request)) = self.interchange.take() {
                // self.state.store(State::Canceled as u8, Ordering::Release);
                self.state.store(State::Idle as u8, Ordering::Release);
                return Ok(Some(request));
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
    /// If the responder has sent a response, we return.
    // It is a logic error to call this method if we're Idle or Canceled, but
    // it seems unnecessary to model this.
    pub fn response(&mut self) -> Option<Response> {
        if self.state.compare_exchange(
            State::Responded as u8,
            State::Idle as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).is_ok() {
            if let Some(Interchange::Response(response)) = self.interchange.take() {
                return Some(response);
            }
            unreachable!();
        }

        None
    }

}

impl Responder {

    #[inline]
    pub fn state(&self) -> State {
        State::from(self.state.load(Ordering::Acquire))
    }

    // If there is a request waiting, take it out
    pub fn request(&mut self) -> Option<Request> {
        if self.state.compare_exchange(
            State::Requested as u8,
            State::Processing as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ).is_ok() {
            if let Some(Interchange::Request(request)) = self.interchange.take() {
                return Some(request);
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

    pub fn respond(&mut self, response: Response) -> Result<(), Response> {
        if State::Processing == self.state.load(Ordering::Acquire) {
            *self.interchange = Some(Interchange::Response(response));
            if self.state.compare_exchange(
                State::Processing as u8,
                State::Responded as u8,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ).is_ok() {
                return Ok(());
            } else {
                // requester canceled in the mean time
                if let Some(Interchange::Response(response)) = self.interchange.take() {
                    return Err(response);
                }
                unreachable!();
            }
        }

        // logic error
        Err(response)
    }

}

pub fn claim() -> Option<(Requester, Responder)> {
    static CLAIMED: AtomicBool = AtomicBool::new(false);
    if CLAIMED
        .compare_exchange_weak(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        static mut INTERCHANGE: Option<Interchange> = None;
        static STATE: AtomicU8 = AtomicU8::new(State::Idle as u8);

        use core::mem::MaybeUninit;
        use core::cell::UnsafeCell;
        unsafe {
            let mut cell: MaybeUninit<UnsafeCell<&'static mut Option<Interchange>>> = MaybeUninit::uninit();
            cell.as_mut_ptr().write(UnsafeCell::new(&mut INTERCHANGE));
            Some((
                Requester {
                    interchange: *(*cell.as_mut_ptr()).get(),
                    state: &STATE,
                },

                Responder {
                    interchange: *(*cell.as_mut_ptr()).get(),
                    state: &STATE,
                },
            ))
        }
    } else {
        None
    }
}
