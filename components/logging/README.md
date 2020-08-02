# Logging

This crate handles logging for the Solo Bee project.  It can run in `no_std` (default) or `std`.

In `no_std`, it uses both `cortex-m-semihosting` and `cortex-m-funnel` crates to do logging.  With `std`, the standard `println` is used.

# API

The following will make non-blocking logs.  Note for these logs to be output, you need to "drain" them.  See how to do that in the `cortex-m-funnel` crate.  This doesn't apply for `std`.

```
info!("log {}", a_value);
warn!("log {}", a_value);
debug!("log {}", a_value);
error!("log {}", a_value);
```

The following will log and block until all information has been output.

```
blocking::info!("log {}", a_value);
blocking::warn!("log {}", a_value);
blocking::debug!("log {}", a_value);
blocking::error!("log {}", a_value);
```

# Configuring

By default, all logs are turned off.  You can use the following features to enable them.

* `all`
* `info`
* `warn`
* `debug`
* `error`

# Enabling/disabling logs at crate level

A crate can generate a wrapper around the log API and use default feature flags to enable/disable them, independent of other crates using `logging`.

Usage:

```rust
logging::add!(my_logger);
// ...
fn my_crate_function(arg: u32){
    my_logger::info!("arg = {}", arg);
}
```

Cargo.toml:

```toml
[features]
log-all = []
log-none = []
log-info = []
log-debug = []
log-warn = []
log-error = []
```

Now logs can be enabled or disabled in this sub crate by passing a feature flag, e.g. `mycrate/log-all` or `mycrate/log-debug`.

# Testing

You will need to change your `--target` based on what your machine is.  See `rustc --print target-list`.

```
cargo test --features std,all --target x86_64-apple-darwin -- --nocapture
```


