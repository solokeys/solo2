# LPC55 runner

The entire firmware that runs all the things.


### Logging

The easy + fast way to log is to use the `log-rtt` feature.
Listening on port `19021` (e.g. via `netcat localhost 19021`) outputs the RTT message output
from `JLinkGDBServer -strict -device LPC55S69 -if SWD -vd`.

The slower alternative (although not so bad due to `delog` bundling) is to use the `log-semihosting` feature.
Both at once does not work, neither does `log-serial`.

Additionally, logging features need to be turned on.
An example invocation: `cargo run --release --features board-lpcxpresso55,develop,log-rtt,fido-authenticator/log-all` 
