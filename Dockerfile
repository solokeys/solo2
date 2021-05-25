FROM rust:latest
RUN apt-get update && \
    apt-get install -y python3 curl llvm clang libclang-dev gcc-arm-none-eabi libc6-dev-i386
WORKDIR /app

# run docker build -f ./Dockerfile -t solo2 .