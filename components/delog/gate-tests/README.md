# local log tests

`delog` has functionality to turn off logs on a library level at compile time.

While we hope to find a way to test this systematically, for now you can do:

```
cargo run
cargo run --features lib-a/log-all,lib-b/log-all
cargo run --features lib-a/log-all
cargo run --features lib-b/log-all
cargo run --features lib-a/log-trace,lib-b/log-error
```
