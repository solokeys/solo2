.PHONY: all clean

all: 
	cargo build --target thumbv7em-none-eabihf --features board-nrfdk
	cargo build --target thumbv8m.main-none-eabi --features board-nk3xn

clean:
	rm -rf target
