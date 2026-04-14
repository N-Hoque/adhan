# syntax=docker/dockerfile:1

# =============================================================================
# Stage 1 – Builder
# =============================================================================
# BuildKit automatically sets BUILDPLATFORM (the host platform running the
# build) and TARGETPLATFORM (the platform the image is being built for).
# We pin the build stage to the *host* platform so Rust and Cargo run natively
# at full speed, then cross-compile to the target platform using the
# appropriate Rust target triple.
# =============================================================================

FROM --platform=$BUILDPLATFORM rust:slim AS builder

ARG TARGETPLATFORM

# Install cross-compilation toolchains and the ALSA headers needed by rodio.
# The conditional logic maps Docker's TARGETPLATFORM to:
#   - The Rust target triple
#   - The apt cross-compiler package
#   - The apt multiarch ALSA package and its sysroot
RUN <<EOF
set -eux

apt-get update
apt-get install -y --no-install-recommends \
    pkg-config

case "$TARGETPLATFORM" in
  linux/amd64)
    RUST_TARGET="x86_64-unknown-linux-gnu"
    apt-get update
    apt-get install -y --no-install-recommends \
        libasound2-dev \
        libpipewire-0.3-dev \
        libclang-dev
    ;;
  linux/arm64)
    RUST_TARGET="aarch64-unknown-linux-gnu"
    dpkg --add-architecture arm64
    apt-get update
    apt-get install -y --no-install-recommends \
        gcc-aarch64-linux-gnu \
        libc6-dev-arm64-cross \
        libasound2-dev:arm64 \
        libpipewire-0.3-dev:arm64 \
        libclang-dev
    # Tell the linker to use the arm64 cross-linker
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
    ;;
  linux/arm/v7)
    RUST_TARGET="armv7-unknown-linux-gnueabihf"
    dpkg --add-architecture armhf
    apt-get update
    apt-get install -y --no-install-recommends \
        gcc-arm-linux-gnueabihf \
        libc6-dev-armhf-cross \
        libasound2-dev:armhf \
        libpipewire-0.3-dev:armhf \
        libclang-dev
    export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc
    ;;
  *)
    echo "Unsupported TARGETPLATFORM: $TARGETPLATFORM" >&2
    exit 1
    ;;
esac

rm -rf /var/lib/apt/lists/*

echo "$RUST_TARGET" > /rust_target.txt
EOF

# Re-read the computed target triple and set the cross-linker env so the
# subsequent RUN steps can use them.  We write them to a small shell fragment
# that every subsequent step sources.
RUN <<EOF
set -eux

RUST_TARGET=$(cat /rust_target.txt)

case "$TARGETPLATFORM" in
  linux/arm64)
    echo 'export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc' >> /env.sh
    echo 'export PKG_CONFIG_SYSROOT_DIR=/usr/aarch64-linux-gnu'                                                          >> /env.sh
    echo 'export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig'                                                   >> /env.sh
    echo 'export PKG_CONFIG_ALLOW_CROSS=1'                                                                                >> /env.sh
    echo 'export BINDGEN_EXTRA_CLANG_ARGS=-I/usr/include/pipewire-0.3 -I/usr/include/spa-0.2'                            >> /env.sh
    ;;
  linux/arm/v7)
    echo 'export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc' >> /env.sh
    echo 'export PKG_CONFIG_SYSROOT_DIR=/usr/arm-linux-gnueabihf'                           >> /env.sh
    echo 'export PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig'                    >> /env.sh
    echo 'export PKG_CONFIG_ALLOW_CROSS=1'                                                   >> /env.sh
    echo 'export BINDGEN_EXTRA_CLANG_ARGS=-I/usr/include/pipewire-0.3 -I/usr/include/spa-0.2' >> /env.sh
    ;;
  linux/amd64)
    echo 'export BINDGEN_EXTRA_CLANG_ARGS=-I/usr/include/pipewire-0.3 -I/usr/include/spa-0.2' >> /env.sh
    ;;
  *)
    touch /env.sh
    ;;
esac

rustup target add "$RUST_TARGET"
EOF

WORKDIR /build

# Copy the manifest and fetch dependencies to cache them.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && touch src/lib.rs
RUN cargo fetch --locked

# Copy the library sources to cache them.
COPY src/lib.rs ./src/lib.rs
COPY src/model.rs ./src/model.rs
RUN <<EOF
set -eux
. /env.sh
RUST_TARGET=$(cat /rust_target.txt)
cargo build --profile size --target "$RUST_TARGET"
EOF

# Copy the main file to compile the binary.
COPY src/main.rs ./src/main.rs
RUN <<EOF
set -eux
. /env.sh
RUST_TARGET=$(cat /rust_target.txt)
cargo build --profile size --target "$RUST_TARGET"
cp target/"$RUST_TARGET"/size/adhan /adhan
EOF

# =============================================================================
# Stage 2 – Runtime image
# =============================================================================
# Minimal Debian image; no Rust toolchain, no build headers.
# Only the ALSA *runtime* library is needed – not the -dev headers.
# =============================================================================

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libasound2 \
        libpipewire-0.3-0 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /adhan /usr/local/bin/adhan

# The binary expects an audio directory and settings file placed in the
# platform config directory (~/.config/adhan by default).  Mount or bind
# your config there at runtime, e.g.:
#   docker run --rm -v "$HOME/.config/adhan:/root/.config/adhan" \
#              --device /dev/snd adhan timetable
ENTRYPOINT ["/usr/local/bin/adhan"]
CMD ["--help"]
