r:
	for i in {0..300}; do echo; done; cargo +nightly run --release --features log-semihosting,debug-trussed,debug-fido-authenticator,trussed-semihosting,fido-authenticator-semihosting,semihost-raw-responses --color always 2>&1|less -r
run:
	for i in {0..300}; do echo; done; \
	 cargo run --release \
	 --features log-semihosting,debug-trussed,debug-fido-authenticator,trussed-semihosting,fido-authenticator-semihosting,semihost-raw-responses \
	 --color always 2&>1 | less -r

clippy:
	cargo clippy --all --all-features -- -D warnings

warnings:
	RUSTFLAGS="-D warnings" cargo build --release --bin rtic

