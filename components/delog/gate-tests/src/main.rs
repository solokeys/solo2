#[macro_use]
extern crate delog;

use delog::flushers::StdoutFlusher;

delog!(Delogger, 4096, StdoutFlusher);

static FLUSHER: StdoutFlusher = StdoutFlusher {};

fn main() {
    Delogger::init(delog::LevelFilter::Info, &FLUSHER).expect("all good");
    lib_a::f();
    lib_b::g();
    println!("log attempts: {}", delog::trylogger().unwrap().attempts());
    println!("log successes: {}", delog::trylogger().unwrap().attempts());
    println!("log flushes: {}", delog::trylogger().unwrap().flushes());
    Delogger::flush();
    println!("log attempts: {}", delog::trylogger().unwrap().attempts());
    println!("log successes: {}", delog::trylogger().unwrap().attempts());
    println!("log flushes: {}", delog::trylogger().unwrap().flushes());
}
