# ============================================================
# CivicSense Pi Stream — cross-compile with Docker
#
#   make build            build all targets into ./bin
#   make build TARGETS="aarch64-unknown-linux-gnu"   # one target
#   make build-legacy     fallback for Docker without buildx
#   make clean
#
# Output layout: bin/<target-triple>/{pi_stream,pi_stream_http,pi_stream_udp}
# ============================================================

DOCKER  ?= docker
BUILDER ?= pi-stream-builder
TARGETS ?= armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu

.PHONY: all build build-legacy clean

all: build

# Primary path: buildkit container driver, exports ./bin directly.
# Requires a Docker daemon with buildx (Docker 20.10+ ships it).
build:
	@mkdir -p bin
	@$(DOCKER) buildx inspect $(BUILDER) >/dev/null 2>&1 || \
		$(DOCKER) buildx create --name $(BUILDER) --driver docker-container
	@$(DOCKER) buildx build \
		--builder $(BUILDER) \
		--build-arg TARGETS="$(TARGETS)" \
		--output type=local,dest=bin .
	@echo "==> binaries in ./bin"

# Fallback: works with plain `docker build` (no buildx driver needed).
build-legacy:
	@mkdir -p bin
	$(DOCKER) build --build-arg TARGETS="$(TARGETS)" -t pi-stream-builder .
	$(DOCKER) create --name pi-stream-extract pi-stream-builder >/dev/null
	$(DOCKER) cp pi-stream-extract:/out/. bin/
	$(DOCKER) rm pi-stream-extract >/dev/null
	@echo "==> binaries in ./bin"

clean:
	rm -rf bin
