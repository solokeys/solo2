RUNNER := runners/lpc55
BOARD ?= nk3xn

build-docker-toolchain:
	docker build . -t nitrokey3

docker-build:
	cp Cargo.lock $(RUNNER)/Cargo.lock #TODO remove this
	docker run -i --rm -v $(PWD):/app nitrokey3 make build

docker-objcopy:
	cp Cargo.lock $(RUNNER)/Cargo.lock #TODO remove this
	docker run -i --rm -v $(PWD):/app nitrokey3 make objcopy BOARD=$(BOARD)

docker-size:
	cp Cargo.lock $(RUNNER)/Cargo.lock #TODO remove this
	docker run -i --rm -v $(PWD):/app nitrokey3 make size

build:
	make -C $(RUNNER) build BOARD=$(BOARD)

objcopy:
	make -C $(RUNNER) objcopy BOARD=nk3am

size:
	make -C $(RUNNER) size BOARD=$(BOARD)

bacon:
	make -C $(RUNNER) bacon

run:
	make -C $(RUNNER) run

jlink:
	scripts/bump-jlink
	JLinkGDBServer -strict -device LPC55S69 -if SWD -vd

mount-fs:
	scripts/fuse-bee

umount-fs:
	scripts/defuse-bee
