FROM rust:latest
RUN apt-get update && \
    apt-get install -y python3 curl llvm clang libclang-dev gcc-arm-none-eabi libc6-dev-i386
RUN cargo install flip-link
RUN rustup target add thumbv8m.main-none-eabi
RUN cargo install cargo-binutils
RUN rustup component add llvm-tools-preview
#RUN git clone https://github.com/Nitrokey/solo2.git && cd solo2 && git checkout nitrokey-main
#COPY Cargo.lock solo2/runners/lpc55
# run docker build -f ./Dockerfile -t solo2 .
