macro_rules! impl_request {
    ($(
        $request:ident:
            $(- $name:tt: $type:path)*
    )*)
        => {$(
    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct $request {
        $(
            pub $name: $type,
        )*
    }

    impl From<$request> for Request {
        fn from(request: $request) -> Self {
            Self::$request(request)
        }
    }

    )*}
}

macro_rules! impl_reply {
    ($(
        $reply:ident:
            $(- $name:tt: $type:ty)*
    )*)
        => {$(

    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct $reply {
        $(
            pub $name: $type,
        )*
    }

    impl From<Reply> for $reply {
        fn from(reply: Reply) -> reply::$reply {
            match reply {
                Reply::$reply(reply) => reply,
                _ => { unsafe { unreachable_unchecked() } }
            }
        }
    }

    )*}
}

