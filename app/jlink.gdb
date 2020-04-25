# source .gdb-dashboard

set history save on
set confirm off

# find commit-hash using `rustc -Vv`
# set substitute-path /rustc/b8cedc00407a4c56a3bda1ed605c6fc166655447 /home/nicolas/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust
set substitute-path /rustc/1572c433eed495d0ade41511ae106b180e02851d /home/nicolas/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust

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
