#!/bin/bash -xe

for i in {0..300}; do echo; done; \
  cargo run --release \
  --features log-semihosting,debug-trussed,debug-fido-authenticator,trussed-semihosting,fido-authenticator-semihosting,semihost-raw-responses \
  --color always 2&>1 | less -r

