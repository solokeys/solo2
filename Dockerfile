FROM rust:latest
RUN apt-get update && \
    apt-get install -y python3 git curl llvm clang libclang-dev gcc-arm-none-eabi libc6-dev-i386 make wget
RUN cargo install flip-link cargo-binutils
RUN rustup target add thumbv8m.main-none-eabi
RUN cargo install cargo-binutils
RUN rustup component add llvm-tools-preview
WORKDIR /app
