use interchange::scratch::*;

pub fn test_happy_path(rq: &mut Requester, rp: &mut Responder) {
    assert!(rq.state() == State::Idle);

    let request = Request::This(1, 2);
    assert!(rq.request(request).is_ok());

    let request = rp.request().unwrap();
    println!("rp got request: {:?}", &request);

    let response = Response::There(-1);
    assert!(!rp.is_canceled());
    assert!(rp.respond(response).is_ok());

    let response = rq.response().unwrap();
    println!("rq got response: {:?}", &response);

}

pub fn test_early_cancel(rq: &mut Requester, rp: &mut Responder) {
    assert!(rq.state() == State::Idle);

    let request = Request::This(1, 2);
    assert!(rq.request(request).is_ok());

    println!("responder could cancel: {:?}", &rq.cancel().unwrap().unwrap());

    assert!(rp.request().is_none());
    assert!(State::Idle == rq.state());
}

pub fn test_later_cancel(rq: &mut Requester, rp: &mut Responder) {
    assert!(rq.state() == State::Idle);

    let request = Request::This(1, 2);
    assert!(rq.request(request).is_ok());

    let request = rp.request().unwrap();
    println!("rp got request: {:?}", &request);

    println!("responder could cancel: {:?}", &rq.cancel().unwrap().is_none());

    assert!(rp.is_canceled());
    assert!(rp.acknowledge_cancel().is_ok());
    assert!(State::Idle == rq.state());
}

pub fn main() {
    let (mut requester, mut responder) = claim().unwrap();

    test_happy_path(&mut requester, &mut responder);
    test_early_cancel(&mut requester, &mut responder);
    test_later_cancel(&mut requester, &mut responder);

}
