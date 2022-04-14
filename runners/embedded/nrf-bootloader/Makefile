
IMAGE_NAME := nrf-toolchain 
DOCKER_USER := $(shell id -u):$(shell id -g)


all: $(IMAGE_NAME).built
	docker run -it \
		--user $(DOCKER_USER) \
		-v $(shell pwd):/work \
		$(IMAGE_NAME) \
		make -f Makefile.docker

enter: 
	docker run -it \
		--user $(DOCKER_USER) \
		-v $(shell pwd):/work \
		$(IMAGE_NAME) \
		bash


$(IMAGE_NAME).built: Dockerfile
	docker build -t $(IMAGE_NAME) .
	touch $<

clean:
	rm -rf build
	rm -f mbr.hex bootloader.hex $(IMAGE_NAME).built output.zip

.PHONY: all enter clean
