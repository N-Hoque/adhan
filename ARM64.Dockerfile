FROM ghcr.io/cross-rs/aarch64-unknown-linux-gnu:latest

ENV PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig

RUN dpkg --add-architecture arm64
RUN apt update && apt upgrade -y
RUN apt install -y libasound2:arm64 libasound2-dev:arm64 gcc-aarch64-linux-gnu