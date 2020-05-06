mod macros;

/// State of the pipes from the point of view of the requester.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum State {
    /// Sending requests is possible
    Idle = 0,
    /// Request is ready for processing, no reply yet
    UnhandledRequest = 1,
    /// Sending replies is possible
    Processing = 2,
    /// Response is ready for use
    ResponseReady = 3,
}

/// Do NOT implement this yourself! Use the macro `interchange!`.
pub trait Interchange {
    type REQUEST: Clone;
    type RESPONSE: Clone;
    /// This is the constructor for a `(RequestPipe, ResponsePipe)` pair.
    ///
    /// The first time it is called in the program, it constructs
    /// singleton static resources, thereafter, `None` is returned.
    fn claim() -> Option<(RequestPipe<Self>, ResponsePipe<Self>)>
        where Self: Sized;

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
}

/// Requesting end of the RPC interchange.
///
/// The owner of this end initiates RPC by sending a request.
/// It must then wait until the receiving end responds, upon which
/// it can send a new request again.
pub struct RequestPipe<I: 'static + Interchange + Sized> {
    // todo: get rid of this publicity
    #[doc(hidden)]
    pub interchange: &'static mut I,
    #[doc(hidden)]
    pub state: &'static mut State,
}

unsafe impl<I: Interchange> Send for RequestPipe<I> {}

/// Processing end of the RPC interchange.
///
/// The owner of this end must eventually reply to any requests made to it.
pub struct ResponsePipe<I: 'static + Interchange + Sized> {
    #[doc(hidden)]
    pub interchange: &'static mut I,
    #[doc(hidden)]
    pub state: &'static mut State,
}

unsafe impl<I: Interchange> Send for ResponsePipe<I> {}

impl<I: Interchange> RequestPipe<I> {
    /// Check if the responder has replied.
    #[inline]
    pub fn has_response(&self) -> bool {
        *self.state == State::ResponseReady
    }

    #[inline]
    pub fn state(&self) -> State {
        *self.state
    }

    /// Return some reply reference if the responder has replied,
    /// without consuming it.
    pub fn peek(&self) -> Option<&I::RESPONSE> {
        if let State::ResponseReady = self.state {
            Some(unsafe { self.interchange.rp_ref() } )
        } else {
            None
        }
    }

    pub fn take_response(&mut self) -> Option<I::RESPONSE> {
        if let State::ResponseReady = self.state {
            Some(unsafe { self.interchange.rp_mut().clone() } )
        } else {
            None
        }
    }

    #[inline]
    pub fn may_request(&self) -> bool {
        *self.state == State::ResponseReady || *self.state == State::Idle
    }

    pub fn try_request(&mut self, request: I::REQUEST) -> Result<(), I::REQUEST> {
        if self.may_request() {
            // *self.interchange = I::Request(request);
            *self.interchange = I::from_rq(request);
            // some kind of sequential consistency, so other interrupt
            // can rely on request existing in unreachable branch,
            // once it sees state "UnhandledRequest"
            *self.state = State::UnhandledRequest;
            Ok(())
        } else {
            Err(request)
        }
    }
}

impl<I: Interchange> ResponsePipe<I> {
    pub fn has_request(&self) -> bool {
        *self.state == State::UnhandledRequest
    }

    #[inline]
    pub fn state(&self) -> State {
        *self.state
    }

    pub fn peek(&self) -> Option<&I::REQUEST> {
        if let State::UnhandledRequest = *self.state {
            Some(unsafe { self.interchange.rq_ref() } )
        } else {
            None
        }
    }

    pub fn take_request(&mut self) -> Option<I::REQUEST> {
        if let State::UnhandledRequest = *self.state {
            Some(unsafe { self.interchange.rq_mut().clone() } )
        } else {
            None
        }
    }

    pub fn must_respond(&self) -> bool {
        *self.state == State::UnhandledRequest || *self.state == State::Processing
    }

    pub fn try_respond(&mut self, response: I::RESPONSE) -> Result<(), I::RESPONSE> {
        if self.must_respond() {
            *self.interchange = I::from_rp(response);
            // some kind of sequential consistency, so other interrupt
            // can rely on response existing in unreachable branch,
            // once it sees state "ResponseReady"
            *self.state = State::ResponseReady;
            Ok(())
        } else {
            Err(response)
        }
    }
}

