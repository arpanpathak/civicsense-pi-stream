# syntax=docker/dockerfile:1
# ============================================================
# CivicSense Pi Stream - multi-architecture cross-compile builder
#
# ONE build produces release binaries for every supported Pi:
#
#   armv7-unknown-linux-gnueabihf   Pi Zero 2 W / Pi 2 / Pi 3
#                                   -> Raspbian 32-bit (glibc)
#   aarch64-unknown-linux-gnu       Pi 3 / Pi 4 / Pi 5 / Zero 2 W
#                                   -> Raspberry Pi OS 64-bit
#   x86_64-unknown-linux-gnu        dev machines / CI
#
# Build (run from the repo root; results land in ./bin):
#
#   docker buildx build --output type=local,dest=bin .
#
# Output layout (per target triple):
#   bin/<triple>/pi_stream  bin/<triple>/pi_stream_http  bin/<triple>/pi_stream_udp
#
# Everything is compiled inside one container with pinned toolchains,
# so local machine state can never change the result - that is the
# point of building "dockerized".
# ============================================================

# ---- stage 1: compile ------------------------------------------------
FROM rust:1-bookworm AS builder

# The glibc-based targets we produce. Override to build just one:
#   --build-arg TARGETS="aarch64-unknown-linux-gnu"
ARG TARGETS="armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu"

# Cross linkers: armv7 and aarch64 need their gcc; x86_64 uses plain gcc.
# (The linker names are matched in .cargo/config.toml.)
#
# IMPORTANT: the libc6-dev-*-cross packages are only *Recommends* of the
# gcc-*-cross packages. With --no-install-recommends they are skipped and
# the linker fails with "cannot find Scrt1.o / crti.o". They own the ARM
# startup files (Scrt1.o, crti.o, crtn.o) and headers, so install them
# explicitly.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
         gcc \
         gcc-arm-linux-gnueabihf \
         gcc-aarch64-linux-gnu \
         libc6-dev-armhf-cross \
         libc6-dev-arm64-cross \
    && rm -rf /var/lib/apt/lists/*

# Register the cross-compilation targets with rustup.
# (x86_64 is the container's native target and needs no registration.)
RUN rustup target add $TARGETS

WORKDIR /src
COPY . .

# Build every target, then copy ONLY the three release binaries into
# /out/<triple>/ so the final image stays small and fast to extract.
RUN set -eux; \
    for t in $TARGETS; do \
        cargo build --release --target "$t"; \
        mkdir -p "/out/$t"; \
        cp -v \
            "target/$t/release/pi_stream" \
            "target/$t/release/pi_stream_http" \
            "target/$t/release/pi_stream_udp" \
            "/out/$t/"; \
    done

# ---- stage 2: export --------------------------------------------------
# A scratch image whose root is /out, so `docker buildx build
# --output type=local,dest=bin` drops the binaries straight into ./bin.
FROM scratch AS artifact
COPY --from=builder /out /
