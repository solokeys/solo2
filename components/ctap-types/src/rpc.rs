use crate::authenticator::{Error, Request, Response};

// PRIOR ART:
// https://xenomai.org/documentation/xenomai-2.4/html/api/group__native__queue.html
// https://doc.micrium.com/display/osiiidoc/Using+Message+Queues

interchange::interchange! {
    CtapInterchange: (Request, Result<Response, Error>)
}

