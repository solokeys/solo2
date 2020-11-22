#[macro_use]
extern crate delog;

use delog::flushers::StdoutFlusher;

delog!(Delogger, 25, StdoutFlusher);

static STDOUT_FLUSHER: StdoutFlusher = StdoutFlusher {};

fn main() {
    Delogger::init(log::LevelFilter::Info, &STDOUT_FLUSHER).ok();

    let msg = "1234567890";

    (0..10).for_each(|i| {
        info!("{}", msg);
        Delogger::flush();
    });
}
