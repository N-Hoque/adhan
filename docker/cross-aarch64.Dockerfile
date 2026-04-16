# Custom cross image for aarch64-unknown-linux-gnu
#
# The official cross images are Ubuntu 20.04 (Focal) based. Focal's glibc is
# 2.31, but PipeWire 0.3.x requires libc6 >= 2.34. This means we cannot
# install libpipewire-0.3-dev:arm64 into the Focal cross sysroot.
#
# Solution: build from debian:bookworm (glibc 2.36). Bookworm ships
# libpipewire-0.3-dev:arm64 (0.3.65) with no glibc conflicts.
#
# Env vars (PKG_CONFIG_PATH, PKG_CONFIG_SYSROOT_DIR, BINDGEN_EXTRA_CLANG_ARGS)
# are injected by cross via the passthrough entries in Cross.toml.

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

# ── 2. arm64 cross-compiler ───────────────────────────────────────────────────
RUN dpkg --add-architecture arm64 && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        gcc-aarch64-linux-gnu \
        g++-aarch64-linux-gnu \
        libc6-dev-arm64-cross \
    && rm -rf /var/lib/apt/lists/*

# ── 3. arm64 sysroot headers ──────────────────────────────────────────────────
# libasound2-dev:arm64       — ALSA headers for rodio/cpal
# libpipewire-0.3-dev:arm64  — PipeWire headers
# libclang-dev               — native (amd64) clang headers for pipewire-rs bindgen
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        libasound2-dev:arm64 \
        libpipewire-0.3-dev:arm64 \
        libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# ── 4. Linker + compiler env for aarch64 ─────────────────────────────────────
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
