# Custom cross image for armv7-unknown-linux-gnueabihf
#
# The official cross images are Ubuntu 20.04 (Focal) based. Focal's glibc is
# 2.31, but PipeWire 0.3.x requires libc6 >= 2.34. This means we cannot
# install libpipewire-0.3-dev:armhf into the Focal cross sysroot.
#
# Solution: build from debian:bookworm (glibc 2.36). Bookworm ships
# libpipewire-0.3-dev:armhf (0.3.65) with no glibc conflicts.

FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

# ── 1. Native build tools ─────────────────────────────────────────────────────
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        gcc \
        libc-dev \
        pkg-config \
        make \
        git \
    && rm -rf /var/lib/apt/lists/*

# ── 2. armhf cross-compiler ───────────────────────────────────────────────────
RUN dpkg --add-architecture armhf && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        gcc-arm-linux-gnueabihf \
        g++-arm-linux-gnueabihf \
        libc6-dev-armhf-cross \
    && rm -rf /var/lib/apt/lists/*

# ── 3. armhf sysroot headers ──────────────────────────────────────────────────
# libasound2-dev:armhf       — ALSA headers for rodio/cpal
# libpipewire-0.3-dev:armhf  — PipeWire headers
# libclang-dev               — native (amd64) clang headers for pipewire-rs bindgen
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        libasound2-dev:armhf \
        libpipewire-0.3-dev:armhf \
        libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# ── 4. Linker + compiler env for armv7 ───────────────────────────────────────
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CC_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc \
    CXX_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++
