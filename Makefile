RUNNER := runners/lpc55

.PHONY: \
	build-dev \
	bacon \
	run-dev \
	jlink \
	mount-fs \
	umount-fs \
	check \
	check-fmt \
	check-clippy

build-dev:
	make -C $(RUNNER) build-dev

bacon:
	make -C $(RUNNER) bacon

run-dev:
	make -C $(RUNNER) run-dev

jlink:
	scripts/bump-jlink
	JLinkGDBServer -strict -device LPC55S69 -if SWD -vd

mount-fs:
	scripts/fuse-bee

umount-fs:
	scripts/defuse-bee

check: check-fmt check-clippy

check-fmt:
	cargo fmt --all --check

check-clippy:
	cargo clippy --all-targets -- -D warnings
	cd runners/lpc55 && cargo clippy --release --features board-lpcxpresso55 -- -D warnings
	cd runners/lpc55 && cargo clippy --release --features board-solo2 -- -D warnings
	cd runners/lpc55 && cargo clippy --release --features board-lpcxpresso55,provisioner-app,admin-app,provisioner-app/test-attestation -- -D warnings

