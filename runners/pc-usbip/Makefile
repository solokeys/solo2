APPNAME=usbip-simulation
APPPATH=target/debug/usbip-simulation

all: | start-sim attach help

.PHONY:
help:
	@echo "###################################################"
	@echo "Done. Device should be visible in your system now. Run 'make stop' to disconnect it."

.PHONY: start-sim
start-sim: $(APPNAME)
	-$(MAKE) stop
	cp $(APPPATH) ./$(APPNAME) -v
	env RUST_LOG=debug ./$(APPNAME) &
	sleep 1

.PHONY: attach
attach: 
	lsmod | grep vhci-hcd || sudo modprobe vhci-hcd
	sudo usbip list -r "localhost"
	sudo usbip attach -r "localhost" -b "1-1"
	sudo usbip attach -r "localhost" -b "1-1"
	sleep 5

.PHONY: build
build: $(APPNAME)

.PHONY: $(APPNAME)
$(APPNAME):
	cargo build

.PHONY: stop
stop:
	sudo usbip detach -p "00"
	killall $(APPNAME)

.PHONY: stop
setup-fedora:
	sudo dnf install usbip

.PHONY: clean
clean:
	cargo clean
	rm $(APPNAME) -v
