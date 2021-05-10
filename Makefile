RUNNER := runners/lpc55

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
