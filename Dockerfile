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

case "$TARGETPLATFORM" in
  linux/amd64)
    RUST_TARGET="x86_64-unknown-linux-gnu"
    apt-get update
    apt-get install -y --no-install-recommends \
        libasound2-dev
    ;;
  linux/arm64)
    RUST_TARGET="aarch64-unknown-linux-gnu"
    dpkg --add-architecture arm64
    apt-get update
    apt-get install -y --no-install-recommends \
        gcc-aarch64-linux-gnu \
        libasound2-dev:arm64
    # Tell the linker to use the arm64 cross-linker
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
    ;;
  linux/arm/v7)
    RUST_TARGET="armv7-unknown-linux-gnueabihf"
    dpkg --add-architecture armhf
    apt-get update
    apt-get install -y --no-install-recommends \
        gcc-arm-linux-gnueabihf \
        libasound2-dev:armhf
    export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc
    ;;
  *)
    echo "Unsupported TARGETPLATFORM: $TARGETPLATFORM" >&2
    exit 1
    ;;
esac

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
    echo 'export PKG_CONFIG_SYSROOT_DIR=/usr/aarch64-linux-gnu'                       >> /env.sh
    echo 'export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig'                >> /env.sh
    echo 'export PKG_CONFIG_ALLOW_CROSS=1'                                             >> /env.sh
    ;;
  linux/arm/v7)
    echo 'export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc' >> /env.sh
    echo 'export PKG_CONFIG_SYSROOT_DIR=/usr/arm-linux-gnueabihf'                           >> /env.sh
    echo 'export PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig'                    >> /env.sh
    echo 'export PKG_CONFIG_ALLOW_CROSS=1'                                                   >> /env.sh
    ;;
  *)
    touch /env.sh
    ;;
esac

rustup target add "$RUST_TARGET"
EOF

WORKDIR /build

# Copy the manifest files first so dependency compilation is cached
# independently of source changes.
COPY Cargo.toml Cargo.lock ./

# Build a dummy main so Cargo fetches and compiles all dependencies.
RUN <<EOF
set -eux
. /env.sh
RUST_TARGET=$(cat /rust_target.txt)
mkdir -p src
echo 'fn main() {}' > src/main.rs
# Touch lib.rs / model.rs so Cargo doesn't complain about missing files
touch src/lib.rs src/model.rs
cargo build --profile size --target "$RUST_TARGET"
# Remove the dummy artefacts so the real source compilation isn't skipped
rm -f target/"$RUST_TARGET"/size/adhan* target/"$RUST_TARGET"/size/deps/adhan*
EOF

# Now copy the real source tree and compile the actual binary.
COPY src ./src

RUN <<EOF
set -eux
. /env.sh
RUST_TARGET=$(cat /rust_target.txt)
cargo build --profile size --target "$RUST_TARGET"
# Copy the binary to a fixed location regardless of target triple
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
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /adhan /usr/local/bin/adhan

# The binary expects an audio directory and settings file placed in the
# platform config directory (~/.config/adhan by default).  Mount or bind
# your config there at runtime, e.g.:
#   docker run --rm -v "$HOME/.config/adhan:/root/.config/adhan" \
#              --device /dev/snd adhan timetable
ENTRYPOINT ["/usr/local/bin/adhan"]
CMD ["--help"]
