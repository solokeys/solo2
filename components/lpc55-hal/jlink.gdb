# source .gdb-dashboard

set history save on
set confirm off

# find commit-hash using `rustc -Vv`
set substitute-path /rustc/cb75ad5db02783e8b0222fee363c5f63f7e2cf5b /home/nicolas/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust

target extended-remote :2331
load
monitor reset

monitor semihosting enable
# monitor semihosting breakOnError <digit>
# by default (1) output goes to Telnet client, 2 sends to GDB client, 3 would send to both
monitor semihosting IOClient 3

# monitor swo enabletarget 0 0 1 0

continue
