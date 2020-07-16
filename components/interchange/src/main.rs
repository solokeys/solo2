//! https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=c6d4a3b396ca318c370d77b204d16a39
//!
//! Motivation: Allow two components to communicate through
//! a request-response "channel", where the responder must
//! respond to all requests before the requester may send
//! a new request.
//!
//! Intended use cases:
//! - RequestPipe is a USB device class that handles de-packetization
//! for some protocol, does some basic low-level error handling
//! and decodes into specific requests that are modeled as Rust structs.
//! ResponsePipe is the main program logic that runs in the RTIC idle loop.
//! - ResponsePipe is an OS service that provides for instance persistent
//! flash storage, or cryptographic services.
//!
//! What this replaces: Two split heapless::spsc::Queues,
//! one each for requests and replies.
//!
//! How is it supposed to work? Both sides share &mut on the underlying
//! buffer (*gasp*). They however also share a controlled view on the state
//! (which moves Idle -> UnhandledRequest -> Processing -> ResponseReady -> Idle
//! in a circular fashion) and guarantees
//!
//! Why?! Since the requests and responses can be quite large,
//! and we want to avoid serde *inside* the program, we want
//! to share the underlying memory used to transfer. We do pick off the
//! messages using clone, but at least we have one static and one stack
//! allocation, instead of two static and one stack allocations.
//!
//! Why not pass references?! In the case of OS services, the requester
//! is assumed to be in non-secure (TrustZone) zone, while the responder
//! sit in the secure zone. We want to have a well-defined memory region
//! through which data must flow (note that secure zone may read/write
//! non-secure RAM). This is ~like a server in networking.
//!
//! Extra credit: Can we tie request variants to allowed response variants?
//! While this is a testable implementation correctness issue, in practice
//! it leads to matching on the Response variant with either possible UB
//! or handling of impossible errrors.


use interchange::{Requester, Responder, State, Interchange as _};

#[derive(Clone, Debug, PartialEq)]
// More serious examples: "perform HMAC-SHA256 with given key handle
// on byte buffer", or "make FIDO2 credential with given parameters",
// or "start doing K.I.T.T lights and wait for user to press button,
// timeout after 10 seconds."
pub enum Request {
    This(u8, u32),
    That(i64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Response {
    Here(u8, u8, u8),
    There(i16),
}

interchange::interchange! {
    ExampleInterchange: (Request, Response)
}

// fn main() {

//     let (mut requester, mut responder) = ExampleInterchange::claim().unwrap();

//     let request = Request::This(1, 2);
//     assert!(requester.peek().is_none());
//     assert!(requester.may_request());
//     assert!(responder.peek().is_none());
//     requester.try_request(request).expect("could not request");

//     assert!(responder.has_request());
//     println!("responder received request: {:?}",
//         &responder.take_request().unwrap());

//     let response = Response::There(-1);

//     assert!(responder.must_respond());
//     responder.try_respond(response).expect("could not respond");

//     assert!(requester.has_response());
//     println!("requester received response: {:?}",
//         &requester.take_response().unwrap());

// }

pub fn test_happy_path(
    rq: &mut Requester<ExampleInterchange>,
    rp: &mut Responder<ExampleInterchange>,
) {
    assert!(rq.state() == State::Idle);

    let request = Request::This(1, 2);
    assert!(rq.request(request).is_ok());

    let request = rp.take_request().unwrap();
    println!("rp got request: {:?}", &request);

    let response = Response::There(-1);
    assert!(!rp.is_canceled());
    assert!(rp.respond(response).is_ok());

    let response = rq.take_response().unwrap();
    println!("rq got response: {:?}", &response);

}

pub fn test_early_cancel(
    rq: &mut Requester<ExampleInterchange>,
    rp: &mut Responder<ExampleInterchange>,
) {
    assert!(rq.state() == State::Idle);

    let request = Request::This(1, 2);
    assert!(rq.request(request).is_ok());

    println!("responder could cancel: {:?}", &rq.cancel().unwrap().unwrap());

    assert!(rp.take_request().is_none());
    assert!(State::Idle == rq.state());
}

pub fn test_later_cancel(
    rq: &mut Requester<ExampleInterchange>,
    rp: &mut Responder<ExampleInterchange>,
) {
    assert!(rq.state() == State::Idle);

    let request = Request::This(1, 2);
    assert!(rq.request(request).is_ok());

    let request = rp.take_request().unwrap();
    println!("rp got request: {:?}", &request);

    println!("responder could cancel: {:?}", &rq.cancel().unwrap().is_none());

    assert!(rp.is_canceled());
    assert!(rp.acknowledge_cancel().is_ok());
    assert!(State::Idle == rq.state());
}

pub fn main() {
    let (mut requester, mut responder) = ExampleInterchange::claim().unwrap();

    test_happy_path(&mut requester, &mut responder);
    test_early_cancel(&mut requester, &mut responder);
    test_later_cancel(&mut requester, &mut responder);

}
