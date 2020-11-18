# source .gdb-dashboard

set history save on
set confirm off

# find commit-hash using `rustc -Vv`
set substitute-path /rustc/8d69840ab92ea7f4d323420088dd8c9775f180cd /home/nicolas/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust

target extended-remote :2331
load
monitor reset
monitor semihosting enable
# monitor semihosting breakOnError <digit>
# by default (1) output goes to Telnet client, 2 sends to GDB client, 3 would send to both
monitor semihosting IOClient 3
#break led.rs:67
#continue
#monitor swo enabletarget 0 0 1 0
# mon SWO EnableTarget 0 48000000 1875000 0
continue
# stepi
