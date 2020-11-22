main-test:
	cargo run --release

test:
	@echo "\nTesting debug build\n"
	time cargo run
	@echo "\nTesting release build\n"
	time cargo run --release

size: build
	arm-none-eabi-size $(KERNEL)

build:
	cargo build

build-release:
	cargo build --release

size-release: build-release
	arm-none-eabi-size $(KERNEL_RELEASE)
	arm-none-eabi-size $(KERNEL_RELEASE) -A

run:
	cargo run

run-release:
	# cargo size --features $(IMPL) --release
	arm-none-eabi-size $(KERNEL_RELEASE)
	cargo run --release
