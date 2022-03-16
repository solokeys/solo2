APPNAME=usb-ip-simulation
APPPATH=target/debug/usb-ip-simulation

all: | start-sim attach

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
