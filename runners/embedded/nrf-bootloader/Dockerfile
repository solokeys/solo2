FROM debian:11

RUN apt-get update && apt-get install -y \
	build-essential python3 wget tar perl patch curl gdebi \
	binutils-arm-none-eabi gcc-arm-none-eabi libnewlib-arm-none-eabi

RUN wget https://github.com/nobodywasishere/nrf5-sdk/archive/v17.1.0-ddde560.tar.gz && \
	mkdir -p /opt/ && \
	tar xfzv v17.1.0-ddde560.tar.gz && \
	mv nRF5-SDK-17.1.0-ddde560 /opt/nrf5-sdk && \
	sed -i -e 's@/usr/local/gcc-arm-none-eabi-9-2020-q2-update/bin/@/usr/bin/@g' /opt/nrf5-sdk/components/toolchain/gcc/Makefile.posix

RUN curl -o jlink.deb -d accept_license_agreement=accepted -d non_emb_ctr=confirmed https://www.segger.com/downloads/jlink/JLink_Linux_V762b_x86_64.deb && \
	gdebi jlink.deb

RUN wget https://www.nordicsemi.com/-/media/Software-and-other-downloads/Desktop-software/nRF-command-line-tools/sw/Versions-10-x-x/10-15-0/nrf-command-line-tools-10.15.0_amd.zip && \
	unzip nrf-command-line-tools-10.15.0_amd.zip && \
  tar xvzf nrf-command-line-tools-10.15.0_Linux-amd64.tar.gz && \
  mv nrf-command-line-tools /opt/nrf5x-command-line-tools && \
	ln -s /opt/nrf5x-command-line-tools/bin/nrfjprog /usr/bin && \
	ln -s /opt/nrf5x-command-line-tools/bin/mergehex /usr/bin

RUN wget https://github.com/NordicSemiconductor/pc-nrfutil/releases/download/v6.1.2/nrfutil-linux && \
  mv nrfutil-linux /usr/bin/nrfutil && \
	chmod +x /usr/bin/nrfutil


ENV NRF5_SDK_PATH=/opt/nrf5-sdk
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8


WORKDIR /work





