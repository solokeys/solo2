.PHONY: all clean

all: 
	( cd app-nrf; cargo build --features board-nrfdk )

clean:
	rm -rf app-nrf/target app-lpc/target
